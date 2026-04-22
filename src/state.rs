use std::sync::Arc;

use tokio::sync::Mutex as TokioMutex;

use crate::config::{self, AppSettingsMap, Settings};
use crate::constants;
use crate::database::DbPool;
use crate::events::BroadcastEventBus;
use crate::telegram::bot_polling;

pub struct BotState {
    pub bot_ready: bool,
    pub bot_running: bool,
    pub bot_error: Option<String>,
    pub app_settings: AppSettingsMap,
    pub shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
}

pub struct AppState {
    pub settings: Settings,
    pub tera: tera::Tera,
    pub http_client: reqwest::Client,
    pub db_pool: DbPool,
    pub event_bus: BroadcastEventBus,
    pub bot_state: TokioMutex<BotState>,
    pub settings_lock: TokioMutex<()>,
}

impl AppState {
    pub fn new(
        settings: Settings,
        tera: tera::Tera,
        http_client: reqwest::Client,
        db_pool: DbPool,
        app_settings: AppSettingsMap,
        bot_ready: bool,
    ) -> Self {
        Self {
            settings,
            tera,
            http_client,
            db_pool,
            event_bus: BroadcastEventBus::new(constants::EVENT_BUS_CAPACITY),
            bot_state: TokioMutex::new(BotState {
                bot_ready,
                bot_running: false,
                bot_error: None,
                app_settings,
                shutdown_tx: None,
            }),
            settings_lock: TokioMutex::new(()),
        }
    }
}

pub async fn start_bot(state: Arc<AppState>) -> Result<(), String> {
    let mut bot = state.bot_state.lock().await;
    let token = bot
        .app_settings
        .get("BOT_TOKEN")
        .and_then(|v| v.clone())
        .unwrap_or_default();
    let channel = bot
        .app_settings
        .get("CHANNEL_NAME")
        .and_then(|v| v.clone())
        .unwrap_or_default();

    if token.is_empty() || channel.is_empty() {
        return Err("BOT_TOKEN or CHANNEL_NAME not configured".into());
    }

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let event_bus = state.event_bus.clone();
    let db_pool = state.db_pool.clone();
    let base_url = bot
        .app_settings
        .get("BASE_URL")
        .and_then(|v| v.clone())
        .unwrap_or_default();

    let token_clone = token.clone();
    let channel_clone = channel.clone();
    let http_client = state.http_client.clone();
    tokio::spawn(async move {
        bot_polling::run_bot_polling(
            token_clone,
            channel_clone,
            db_pool,
            event_bus,
            base_url,
            http_client,
            shutdown_rx,
        )
        .await;
    });

    bot.shutdown_tx = Some(shutdown_tx);
    bot.bot_running = true;
    bot.bot_error = None;
    tracing::info!("机器人已在后台启动");
    Ok(())
}

pub async fn stop_bot(state: &AppState) {
    let mut bot = state.bot_state.lock().await;
    if let Some(tx) = bot.shutdown_tx.take() {
        let _ = tx.send(());
    }
    bot.bot_running = false;
    tracing::info!("机器人已停止");
}

pub async fn apply_runtime_settings(
    state: Arc<AppState>,
    start_bot_flag: bool,
) -> Result<(), String> {
    let _lock = state.settings_lock.lock().await;
    let current = config::get_app_settings(&state.settings, &state.db_pool);
    let bot_ready = config::is_bot_ready(&current);

    // Soft refresh path: the caller only wants to pick up updated
    // `app_settings` (e.g. after `/api/auth/login` rotates SESSION_TOKEN).
    // Previously this code stopped the running bot even for soft refreshes,
    // which meant logging in as the admin would silently kill the Telegram
    // bot. Now we only update the in-memory snapshot and leave the bot alone.
    if !start_bot_flag {
        let mut bot = state.bot_state.lock().await;
        bot.app_settings = current;
        bot.bot_ready = bot_ready;
        // Do not clobber an existing bot_error on a soft refresh.
        return Ok(());
    }

    // Hard apply path: stop the bot, swap config, and restart if ready.
    stop_bot(&state).await;

    {
        let mut bot = state.bot_state.lock().await;
        bot.app_settings = current;
        bot.bot_ready = bot_ready;
        bot.bot_error = None;
    }

    if bot_ready {
        if let Err(e) = self::start_bot(state.clone()).await {
            tracing::error!("应用配置已应用，但启动机器人失败: {}", e);
            let mut bot = state.bot_state.lock().await;
            bot.bot_error = Some(e.to_string());
            return Err(e);
        }
    }

    Ok(())
}
