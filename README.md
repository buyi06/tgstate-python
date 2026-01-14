# tgState-Python

**基于 Telegram 的无限私有云存储 & 永久图床系统**

将您的 Telegram 频道或群组瞬间变身为功能强大的私有网盘与图床。无需服务器存储空间，借助 Telegram 的无限云端能力，实现文件管理、外链分享、图片托管等功能。

本项目采用 Python (FastAPI) 开发，界面清爽，部署简单，支持 Web 界面与 Telegram 机器人双向互动。

---

## ✨ 核心功能

*   **☁️ 无限存储**：利用 Telegram 频道作为后端存储，容量无上限。
*   **🖼️ 永久图床**：支持图片上传并生成永久直链，可作为 Markdown 图床使用。
*   **📂 Web 管理**：提供高颜值的 Web 控制台，支持文件列表、搜索、拖拽上传、批量删除。
*   **🔗 高速直链**：为存储的文件生成 HTTP 下载直链，支持浏览器直接下载与在线预览。
*   **🤖 双向同步**：
    *   **Web 上传**：在网页上传的文件会自动转发到 Telegram 频道。
    *   **TG 上传**：直接在 Telegram 频道/群组发送文件，Web 端也会自动同步显示。
*   **🔒 安全隐私**：支持设置 Web 访问密码，保护您的私有数据。
*   **🧩 扩展支持**：兼容 PicGo（通过 web-uploader 插件），支持作为图床上传接口。

---

## ⚠️ 重要提醒与安全建议

1.  **保护好您的 BOT_TOKEN**：这是机器人的唯一凭证，切勿泄露给他人，否则您的频道可能被恶意操作。
2.  **务必设置 Web 密码**：部署后首次访问会强制要求设置管理员密码。由于 Web 端拥有上传和删除文件的权限，强密码是必须的。
3.  **Telegram 限制**：
    *   **单文件大小**：机器人 API 限制上传最大 50MB（本项目主要用于图床和小文件网盘）。直接在 TG 客户端发送的文件大小限制为 2GB（Web 端可读取但无法通过 Bot 下载大文件）。
    *   **API 频率**：请勿进行极高并发的滥用，以免触发 Telegram 的 API 限制。
4.  **数据备份**：虽然 Telegram 存储相对可靠，但对于极端重要的数据，建议保留本地备份。

---

## 🖥️ 界面预览

| 文件管理首页 | 图床模式 | 系统设置 |
| :---: | :---: | :---: |
| ![Web UI Files](https://placehold.co/600x400/f7f9fc/2563eb?text=File+Manager) | ![Web UI Images](https://placehold.co/600x400/f7f9fc/2563eb?text=Image+Hosting) | ![Web UI Settings](https://placehold.co/600x400/f7f9fc/2563eb?text=Settings) |

*（截图占位：部署后您将看到全新的高颜值响应式界面）*

---

## 🚀 一键部署（推荐）

无需下载代码，无需手动安装环境，只需一台安装了 Docker 的服务器（Linux/Windows/Mac 均可）。

**复制以下命令并在终端执行：**

```bash
# 1. (可选) 清理旧容器（如果是更新或重装）
docker rm -f tgstate

# 2. 拉取代码并构建镜像（使用 GitHub 最新代码）
docker build -t tgstate https://github.com/buyi06/tgstate-python.git#main

# 3. 启动容器
# 默认端口为 8000，数据挂载到当前目录的 data 文件夹（可选）
docker run -d \
  --name tgstate \
  --restart always \
  -p 8000:8000 \
  -v $(pwd)/data:/app/data \
  tgstate
```

**✅ 部署成功后：**
请在浏览器访问：`http://您的服务器IP:8000`

> **💡 端口被占用？**
> 如果 `8000` 端口已被其他程序占用，可以修改 `-p` 参数。例如改为 `15767` 端口：
> `docker run -d --name tgstate --restart always -p 15767:8000 tgstate`
> 此时访问地址变为：`http://您的服务器IP:15767`

---

## 🛠️ 首次使用流程

1.  **初始化设置（引导页）**：
    *   部署后首次访问网页，会自动进入 **“引导页”**。
    *   您只需设置 **管理员密码**（用于登录 Web 后台）。
    *   设置完成后，引导页将永久隐藏。

2.  **配置系统参数**：
    *   登录后，点击左侧菜单的 **“系统设置”**。
    *   您需要填写以下核心信息（填写方法见下文教程）：
        *   `BOT_TOKEN`（机器人令牌）
        *   `CHANNEL_NAME`（频道/群组 ID）
        *   `BASE_URL`（您的 Web 访问地址，用于生成直链）
    *   填写完毕后点击 **“保存并应用”**，系统即刻生效。

---

## ⚙️ 配置项说明

| 配置项 | 必填 | 说明 | 示例 |
| :--- | :---: | :--- | :--- |
| **BOT_TOKEN** | ✅ | Telegram 机器人的访问令牌，从 @BotFather 获取 | `123456789:AAH...` |
| **CHANNEL_NAME** | ✅ | 存储文件的频道或群组 ID（建议使用 ID 格式） | `@my_channel` 或 `-100123456789` |
| **PASS_WORD** | ✅ | Web 管理界面的登录密码 | `MyStrongPass123` |
| **BASE_URL** | ⚠️ | **强烈建议填写**。用于生成正确的文件下载/分享直链。 | `http://1.2.3.4:8000` 或 `https://pan.example.com` |
| **PICGO_API_KEY** | ❌ | （可选）仅在使用 PicGo 上传时需要，作为 API 密钥。 | `my_secret_key` |

---

## 📖 参数获取教程（保姆级）

### A. 获取 BOT_TOKEN

1.  在 Telegram 中搜索 **[@BotFather](https://t.me/BotFather)** 并点击“开始”。
2.  发送指令 `/newbot` 创建新机器人。
3.  按提示给机器人起个名字（Name，例如 `MyCloudBot`）。
4.  给机器人起个用户名（Username，**必须以 `bot` 结尾**，例如 `my_cloud_drive_bot`）。
5.  成功后，BotFather 会回复一段话，其中 `Use this token to access the HTTP API:` 下方的那串字符就是 **BOT_TOKEN**。
    *   *格式形如：`123456789:ABCdefGHIjklMNOpqrSTUvwxYZ`*

### B. 准备存储频道/群组

1.  **新建频道/群组**：建议新建一个 **私有频道 (Private Channel)** 专门用于存储文件，避免公开泄露。
2.  **添加机器人**：进入频道/群组的“管理” -> “管理员” -> “添加管理员”，搜索刚才创建的机器人用户名，将其**添加为管理员**（给予所有权限）。
    *   *注意：必须是管理员，否则机器人无法在频道内发送文件或读取消息。*

### C. 获取 CHANNEL_NAME (Chat ID)

推荐使用 **Chat ID**（以 `-100` 开头的数字），比用户名更稳定。

**方法一：使用 @RawDataBot（最简单）**
1.  将您的机器人拉入频道/群组。
2.  在频道/群组中随便发一条消息。
3.  将这条消息**转发**给 **[@RawDataBot](https://t.me/RawDataBot)**。
4.  该机器人会回复一段 JSON 数据，找到 `forward_from_chat` (或 `chat`) 下面的 `id` 字段。
    *   这就是您的 Chat ID，通常以 `-100` 开头，例如 `-1001234567890`。

**方法二：通过网页 API 查看**
1.  确保机器人已在频道中，且您刚在频道里发了一条测试消息。
2.  在浏览器访问：`https://api.telegram.org/bot<您的BOT_TOKEN>/getUpdates`
    *   *将 `<您的BOT_TOKEN>` 替换为实际 Token。*
3.  查看返回的 JSON，寻找 `chat` -> `id` 字段。

---

## 📝 使用说明

### 1. Web 网页版
*   **上传**：直接拖拽文件到首页上传区域，或点击上传按钮。支持批量上传。
*   **管理**：在文件列表中，您可以复制下载直链、复制 Markdown 格式（用于图床）、或删除文件。
*   **搜索**：顶部搜索框支持实时过滤文件名。

### 2. Telegram 客户端
*   **上传**：直接向您绑定的频道/群组发送文件、图片、视频。
*   **同步**：发送成功后，刷新 Web 页面，文件会自动出现在列表中。
*   **指令**：目前主要通过 Web 端管理，Bot 端指令开发中。

### 3. PicGo 图床配置
如果您使用 [PicGo](https://picgo.github.io/PicGo-Doc/) 上传图片，请安装 `web-uploader` 插件。

*   **API 地址**：`http://您的服务器IP:8000/api/upload`
*   **POST 参数名**：`file`
*   **JSON 路径**：`url`
*   **自定义 Header**：如果不设置 API Key 则无需填写；如果设置了 `PICGO_API_KEY`，请添加 Header：`Authorization: 您设置的KEY`

---

## ❓ 常见问题排障 (FAQ)

**Q1: 部署后网页打不开？**
*   检查防火墙/安全组是否放行了 `8000` 端口。
*   检查 Docker 容器是否正在运行：`docker ps`。
*   如果您修改了端口映射（如 `15767:8000`），请确保访问的是 `http://IP:15767`。

**Q2: 网页能打开，但上传文件报错 / 文件列表为空？**
*   **检查 BOT_TOKEN**：确保没有多复制空格。
*   **检查 CHANNEL_NAME**：确保 ID 正确（带 `-100` 前缀），且**机器人必须是该频道的管理员**。
*   **检查网络**：您的服务器必须能访问 `api.telegram.org`。如果服务器在国内，可能需要配置代理（本项目暂未内置代理配置，建议使用海外 VPS）。

**Q3: 点击“下载”或“复制链接”生成的地址不对？**
*   请在“系统设置”中检查 **BASE_URL**。
*   如果它是 `http://localhost:8000`，请将其改为您的**公网 IP** 或 **域名**（例如 `http://1.2.3.4:8000`）。

**Q4: 如何更新到最新版本？**
只需重新执行部署命令的后两步即可：
```bash
docker stop tgstate && docker rm tgstate
docker build -t tgstate https://github.com/buyi06/tgstate-python.git#main
docker run -d --name tgstate --restart always -p 8000:8000 -v $(pwd)/data:/app/data tgstate
```

---

## 📂 项目结构简述

```text
/app
  ├── main.py            # 程序入口 (FastAPI)
  ├── api/               # API 路由处理
  ├── templates/         # 前端 HTML 模板 (Jinja2)
  ├── static/            # 静态资源 (CSS/JS)
  └── core/              # 核心逻辑 (Telegram Bot 交互)
/data                    # 数据持久化目录 (数据库/日志)
Dockerfile               # Docker 构建文件
requirements.txt         # Python 依赖
```

---

## 📄 License

本项目基于 [MIT License](LICENSE) 开源。欢迎 Star 与 Fork！
