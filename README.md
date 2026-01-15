# tgstate-python

**基于 Telegram 的无限私有云存储 & 永久图床系统**

将您的 Telegram 频道或群组瞬间变身为功能强大的私有网盘与图床。无需服务器存储空间，借助 Telegram 的无限云端能力，实现文件管理、外链分享、图片托管等功能。

---

## ✅ 一键安装 / 一键更新（保留数据，推荐）

默认端口 **8000**（最通用）
```bash
docker volume create tgstate-data >/dev/null 2>&1; docker rm -f tgstate >/dev/null 2>&1 || true; docker pull ghcr.io/buyi06/tgstate-python:latest && docker run -d --name tgstate --restart unless-stopped -p 8000:8000 -v tgstate-data:/app/data ghcr.io/buyi06/tgstate-python:latest
```

自定义端口 **15767**（可选）
```bash
docker volume create tgstate-data >/dev/null 2>&1; docker rm -f tgstate >/dev/null 2>&1 || true; docker pull ghcr.io/buyi06/tgstate-python:latest && docker run -d --name tgstate --restart unless-stopped -p 15767:8000 -v tgstate-data:/app/data ghcr.io/buyi06/tgstate-python:latest
```

## 🧨 彻底重装（清空所有数据，不可逆）

```bash
docker rm -f tgstate >/dev/null 2>&1 || true; docker volume rm tgstate-data >/dev/null 2>&1 || true; docker volume create tgstate-data >/dev/null 2>&1; docker pull ghcr.io/buyi06/tgstate-python:latest && docker run -d --name tgstate --restart unless-stopped -p 15767:8000 -v tgstate-data:/app/data ghcr.io/buyi06/tgstate-python:latest
```

---

## ⚙️ 首次配置教程

部署后首次访问网页，会进入“引导页”设置管理员密码。之后请进入 **“系统设置”** 完成核心配置。

### 第一步：获取 BOT_TOKEN
1.  在 Telegram 搜索 **[@BotFather](https://t.me/BotFather)** 并点击“开始”。
2.  发送指令 `/newbot` 创建新机器人。
3.  按提示输入 Name（名字）和 Username（用户名，必须以 `bot` 结尾）。
4.  成功后，BotFather 会发送一条消息，其中 `Use this token to access the HTTP API:` 下方的那串字符就是 **BOT_TOKEN**。

### 第二步：获取 Chat ID (CHANNEL_NAME)
1.  **准备群组/频道**：
    *   您可以新建一个群组或频道（公开或私密均可）。
    *   **关键操作**：必须将您的机器人拉入该群组/频道，并设为**管理员**（给予读取消息和发送消息的权限）。
2.  **获取 ID**：
    *   在群组/频道内随便发送一条文本消息。
    *   在浏览器访问：`https://api.telegram.org/bot<您的Token>/getUpdates`
        *   *请将 `<您的Token>` 替换为实际的 BOT_TOKEN。*
    *   查看返回的 JSON，找到 `chat` 字段下的 `id`。
        *   通常是以 `-100` 开头的数字（例如 `-1001234567890`）。
    *   **如果是公开频道**：也可以直接使用频道用户名（例如 `@my_channel_name`）。

> **💡 提示**：如果 `getUpdates` 返回空 (`"result": []`)，请尝试在群里多发几条消息，或者去 @BotFather 关闭机器人的 Group Privacy 模式（`/mybots` -> 选择机器人 -> Bot Settings -> Group Privacy -> Turn off）。

### 第三步：填写配置
回到网页的“系统设置”，填入：
*   **BOT_TOKEN**: 第一步获取的 Token。
*   **CHANNEL_NAME**: 第二步获取的 Chat ID（推荐使用数字 ID）。
*   **BASE_URL** (可选): 您用于对外分享的域名或 IP（例如 `http://1.2.3.4:8000` 或 `https://pan.example.com`）。
    *   *注意：系统已优化，不填也能自动生成可用的分享链接，但在反向代理环境下，为了 Bot 回复链接的准确性，建议填写。*

保存后即可开始使用！

---

## 🌐 反向代理说明 (Caddy/Nginx)

如果您使用 Caddy/Nginx 等反向代理工具，请注意以下几点：

### 1. Cookie 与 HTTPS
系统已优化 Cookie 策略，支持在 HTTP (IP:Port) 和 HTTPS 环境下自动适配。但如果您在反代层开启了 HTTPS，请确保将请求头正确透传。

### 2. Caddy 配置示例
在您的 `Caddyfile` 中追加以下配置（仅供参考）：

```caddy
buyi.us.ci {
    encode gzip
    reverse_proxy 127.0.0.1:8000
}
```

### 3. Nginx 配置示例
确保透传 `Host` 和 `X-Forwarded-*` 头：

```nginx
location / {
    proxy_pass http://127.0.0.1:8000;
    proxy_set_header Host $host;
    proxy_set_header X-Real-IP $remote_addr;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
    proxy_set_header X-Forwarded-Proto $scheme;
}
```

---

## ❓ 常见问题排查

### Q: 登录后跳转回登录页 / 无法登录？
*   **检查密码**：设置密码时系统会自动去除首尾空格，请确认输入的密码无误。
*   **Cookie 问题**：如果您在本地开发环境使用 `localhost`，通常没问题。如果是 IP 访问，请确保浏览器没有禁用 Cookie。尝试点击浏览器地址栏的小锁/图标查看 Cookie 是否写入。
*   **重置配置**：如果实在无法登录，可以删除 `data/file_metadata.db` 中的 `app_settings` 表记录（需懂 SQL），或直接删除数据库文件（会丢失文件索引，不推荐）。

### Q: 退出登录点击无反应或报错？
*   退出登录使用了 JavaScript 弹窗确认，请确保页面 JS 已加载（查看控制台是否有报错）。
*   如果提示网络错误，请刷新页面重试。

### Q: 复制链接失败？
*   在非 HTTPS 环境下（如 HTTP IP 访问），浏览器可能会限制剪贴板 API。系统已内置回退机制，如果自动复制失败，会弹窗显示链接供您手动复制。
*   建议配置 HTTPS 反代以获得最佳体验。

### Q: 删除文件后列表不刷新？
*   删除操作是异步的。如果删除成功但列表未消失，可能是网络延迟。
*   请尝试刷新页面。如果文件仍在，说明删除失败（可能是 Bot 权限不足，请检查 Bot 是否为频道管理员）。

### Q: 分享链接是 127.0.0.1？
*   系统前端会自动根据您当前的浏览器地址生成分享链接。如果您看到 127.0.0.1，说明您就是通过 127.0.0.1 访问的。
*   请尝试用公网 IP 或域名访问网页，分享链接会自动变更为对应的 IP/域名。

---

## 📂 功能特性
*   **无限存储**：依赖 Telegram 频道，容量无上限。
*   **短链接分享**：生成简洁的分享链接（`/d/AbC123`），自动适配当前访问域名。
*   **拖拽上传**：支持批量拖拽上传，大文件自动分块。
*   **图床模式**：支持 Markdown/HTML 格式一键复制，适配 PicGo。
*   **隐私安全**：所有数据存储在您的私有频道，Web 端支持密码保护。

---

## 📄 License
MIT License
