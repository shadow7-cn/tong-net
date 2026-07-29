# 同网互通

<p align="center">
  <img src="apps/desktop/public/brand/tong-net-logo.png" width="128" alt="同网互通 Logo" />
</p>

同网互通是一款免费、开源的设备聊天与文件传输工具。在同一局域网内，一台 Windows 或 macOS 电脑开启服务后，手机、平板和其他电脑无需安装客户端，直接使用浏览器即可加入。

需要跨网络连接时，可以使用应用内置的 EasyTier Core 接入自建的“同网互通组网服务”。所有消息、文件和记录仍保存在自己的主机电脑中：局域网模式不需要账号、云服务或公网服务器；虚拟局域网模式需要在一台 Linux 服务器上部署组网服务。

## 使用者指南

### 主要功能

- 一键开启或停止局域网服务
- 通过二维码或局域网地址加入
- 浏览器端适配手机、平板和电脑
- 按访问端建立一对一会话
- 实时文字聊天与在线状态
- 多文件上传、进度、速度、取消和重试
- 上传中断后自动清理不完整文件
- 下载支持 HTTP Range 断点续传
- 微信内置浏览器可复制文件下载链接
- 主机端支持选择文件另存路径
- 本地保存访问端、聊天、文件和传输记录
- 可移除离线访问端，历史记录仍然保留
- 可选访问令牌；默认允许可信局域网内直接访问
- 内置 EasyTier，可通过虚拟局域网连接远程设备

### 下载安装

请前往 [Releases](https://github.com/shadow7-cn/tong-net/releases) 下载适合自己电脑的版本：

- Windows：可以选择安装版 `setup.exe`，也可以选择解压后直接使用的 `portable.zip`。
- Apple 芯片 Mac（M1、M2、M3、M4 等）：下载 `aarch64.dmg`。
- Intel 芯片 Mac：下载 `x64.dmg`。

#### macOS 提示“应用已损坏”

当前 macOS 版本尚未完成 Apple 开发者签名和公证，因此从浏览器下载后，系统可能提示“同网互通已损坏，无法打开”。确认安装包来自本项目的 GitHub Releases 后，可以按下面的步骤处理：

1. 打开下载的 DMG，把“同网互通”拖到“应用程序”文件夹。
2. 打开 Mac 的“终端”应用。可以按 `Command + 空格键`，输入“终端”，然后按回车。
3. 复制下面整条命令，粘贴到终端后按回车：

```bash
sudo xattr -cr "/Applications/同网互通.app"
```

4. 根据提示输入 Mac 的登录密码，然后按回车。输入密码时终端不会显示文字或圆点，这是正常现象。
5. 回到“应用程序”文件夹，重新打开“同网互通”。

> [!IMPORTANT]
> 这条命令会移除 macOS 为下载文件添加的安全隔离属性。请只对从本项目官方 GitHub Releases 下载的“同网互通”使用，不要对来源不明的软件执行。

### 使用方式

1. 在 Windows 或 macOS 电脑上打开同网互通。
2. 点击“开启互通”。
3. 其他设备扫描二维码，或在浏览器输入界面显示的局域网地址。
4. 选择一个访问端，开始聊天或传输文件。

浏览器直接访问 `http://主机局域网IP:端口/` 时，会自动进入 Web 端。

#### 通过虚拟局域网连接

1. 在左侧进入“虚拟局域网”。
2. 填写组网服务端完整地址、网络名称、网络密码和自己的设备名称。
3. 点击“连接虚拟局域网”。
4. 连接成功后，回到 App 端首页，切换到虚拟局域网地址或二维码。
5. 其他已经加入同一 EasyTier 网络的设备，可以通过该地址访问同网互通。

服务端地址示例为 `https://vpn.example.com` 或 `http://服务器IP:17280`。使用 HTTP 时，桌面端会在首次连接前提示风险，并持续显示“未加密”标签。

组网服务支持两种模式：

- **私有节点**：管理员在服务端创建网络。用户填写可见的网络名称和密码，服务端为每台设备签发独立凭据，支持按网络撤销设备。
- **公共节点**：用户自行填写网络名称和密码，服务端只提供共享节点，不保存这些网络信息。

#### 首次启用虚拟局域网

首次连接虚拟局域网时，系统会请求一次管理员权限，用于安装“同网互通 EasyTier 服务”。安装完成后，连接和断开虚拟局域网不再重复要求管理员密码。

- macOS 会安装系统 LaunchDaemon。
- Windows 会安装 Windows Service。
- 同网互通本身仍以普通用户权限运行，只有 EasyTier 后台服务拥有创建虚拟网卡所需的权限。
- 退出同网互通后，后台服务会自动结束当前 EasyTier Core，不会继续保持虚拟网络连接。

> [!WARNING]
> 默认开启无令牌访问，任何能够访问主机物理局域网或 EasyTier 虚拟地址的设备都可以连接、聊天和传输文件。请仅在可信网络中使用；启用虚拟局域网或连接公共 Wi-Fi 前，建议先在设置里关闭“允许无令牌访问”。普通局域网访问本身不加密，跨网络传输由 EasyTier 隧道提供保护。

### 数据保存位置

- 收到的文件：默认保存在系统下载目录下的 `同网互通` 文件夹，可在设置中修改。
- 聊天和传输记录：保存在主机本地 SQLite 数据库中。
- 未完成上传：保存在临时目录，失败或取消时自动清理。

同网互通不会主动上传遥测数据，也没有配套云端服务。卸载或清理应用数据前，请自行备份需要保留的记录和文件。

### 当前限制

- 上传不支持断点续传；下载支持断点续传。
- 不支持群聊和广播；跨网络互通需要自行部署同网互通组网服务。
- 不提供用户账号、云同步和远程备份。
- 同一设备使用不同浏览器或微信内置浏览器访问时，会被识别为不同访问端。
- 浏览器和操作系统可能限制后台传输与下载行为。

---

## 自建组网服务

这一部分面向拥有 Linux 服务器、希望跨网络使用同网互通的用户。当前版本需要从项目代码构建 Docker 镜像，镜像市场安装将在后续版本提供。

### 准备条件

- 一台可以运行 Docker 和 Docker Compose 的 Linux 服务器。
- 放通 `17280/TCP`，用于管理页面和桌面端认证。
- 放通 `11010/TCP` 和 `11010/UDP`，用于 EasyTier 组网。
- 服务器存在 `/dev/net/tun`。

两个端口都是默认值，可以在 `deploy/.env` 中修改。

### 部署

```bash
git clone https://github.com/shadow7-cn/tong-net.git
cd tong-net
cp deploy/.env.example deploy/.env
docker compose -f deploy/docker-compose.yml build
docker compose -f deploy/docker-compose.yml up -d
```

构建完成后，在浏览器打开：

```text
http://服务器IP:17280
```

首次进入会要求创建管理员用户名和密码、填写站点名称和对外 IP 或域名，并选择公共或私有节点模式。私有模式还需要创建第一个网络。

常用管理命令：

```bash
# 查看运行状态
docker compose -f deploy/docker-compose.yml ps

# 查看日志
docker compose -f deploy/docker-compose.yml logs -f

# 停止服务
docker compose -f deploy/docker-compose.yml down

# 更新代码后重新构建
git pull
docker compose -f deploy/docker-compose.yml up -d --build
```

### 忘记管理员密码

在服务器上执行下面的命令，然后根据提示输入两次新密码：

```bash
docker exec -it tong-net-server tong-net-server admin reset-password
```

重置后，已经登录的管理页面会全部退出，需要使用新密码重新登录。

### 数据备份

数据库、密钥、网络配置和设备凭据都保存在 `deploy/data`。备份前建议先停止容器：

```bash
docker compose -f deploy/docker-compose.yml down
tar -czf tong-net-server-backup.tar.gz deploy/data
docker compose -f deploy/docker-compose.yml up -d
```

恢复时，应把完整的 `deploy/data` 一起恢复，不能只复制 SQLite 数据库，否则已加密的网络配置将无法解密。

> [!IMPORTANT]
> 直接通过公网使用 `http://` 会明文传输登录信息。正式使用建议配置域名，并通过 Caddy、Nginx 或 1Panel 反向代理提供 HTTPS。组网端口 `11010/TCP+UDP` 仍需直接转发到容器。

---

## 开发与贡献者指南

以下内容面向希望了解实现方式、参与开发或自行构建同网互通的开发者。

### 工作原理

同网互通采用主机中转模式，而不是浏览器之间直接 P2P 连接：

```text
浏览器访问端 A ─┐
                 ├─ HTTP / WebSocket ─ 桌面主机 ─ SQLite + 本地文件
浏览器访问端 B ─┘
```

- Tauri 桌面应用负责启动和停止局域网服务。
- React Web 客户端由主机直接提供给其他设备。
- REST API 处理设备、消息、文件和记录。
- WebSocket 推送消息及在线状态变化。
- 文件先完整上传到临时目录，成功后再移入保存目录。
- 下载接口支持 `Range` 请求，可由浏览器继续未完成的下载。
- 接入 EasyTier 后，同一套 HTTP / WebSocket 服务也可以通过 EasyTier 虚拟 IP 访问。

EasyTier Core 需要创建虚拟网卡，因此通过独立系统服务管理：

```text
同网互通（普通用户）
        │ 本机鉴权 IPC：127.0.0.1:17283
        ▼
EasyTier 系统服务（管理员权限）
        │ 启动、停止和监控
        ▼
EasyTier Core
```

- macOS 使用 LaunchDaemon，Windows 使用 Windows Service。
- 首次连接时安装服务，后续连接和断开无需重复授权。
- App 退出或异常结束后，系统服务会停止本次启动的 EasyTier Core。

### 技术栈

- Tauri 2
- Rust、Axum、Tokio
- React、TypeScript、Vite
- Ant Design、Less、Lucide
- Zustand、Axios
- SQLite
- EasyTier Core
- macOS LaunchDaemon、Windows Service

项目采用 npm workspaces 组织：

- `apps/desktop`：Tauri 桌面端及浏览器访问端。
- `apps/server`：Linux 组网服务 Rust 后端。
- `apps/server-web`：Linux 管理 Web。
- `docker`、`deploy`：服务端镜像和 Compose 配置。

### 本地开发

#### 环境要求

- Node.js 20 或更高版本
- npm
- Rust stable
- Tauri 2 对应平台依赖

具体系统依赖请参考 [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/)。

#### 安装依赖

```bash
npm install
```

#### 启动桌面开发环境

```bash
npm run tauri -- dev
```

#### 运行测试

```bash
npm test -w desktop
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
cargo test --manifest-path apps/server/Cargo.toml
```

#### 构建前端

```bash
npm run build
npm run build:server-web
```

#### 打包桌面应用

```bash
npm run prepare:easytier
npm run tauri -- build
```

`prepare:easytier` 会下载当前平台对应的 EasyTier Core，并在打包前完成校验。产物位于 `apps/desktop/src-tauri/target/release/bundle/`。

建议在目标操作系统上分别打包：Windows 生成 NSIS 安装包，macOS 生成 App/DMG。跨平台正式产物可以交给下方的 GitHub Actions 工作流构建。

### 发布新版本

仓库提供 GitHub Actions 发布工作流。推送与应用版本一致的 `v*` 标签后，会自动生成：

- Windows x64 NSIS 安装版 `setup.exe`
- Windows x64 便携版 `portable.zip`
- macOS Apple Silicon `dmg`
- macOS Intel `dmg`

发布前需要同步修改以下版本号：

- `apps/desktop/src-tauri/tauri.conf.json`
- `apps/desktop/src-tauri/Cargo.toml`
- 根目录和桌面 workspace 的 `package.json`

桌面端和组网服务端统一使用同一个版本号。修改版本号后，执行一次 `npm install --package-lock-only`，让 `package-lock.json` 同步更新。以发布 `0.2.0` 为例：

```bash
npm install --package-lock-only
npm run prepare:easytier
npm test -w desktop
npm run build
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml

git add README.md package.json package-lock.json apps/desktop/package.json \
  apps/desktop/src-tauri/Cargo.toml apps/desktop/src-tauri/Cargo.lock \
  apps/desktop/src-tauri/tauri.conf.json
git commit -m "chore: release v0.2.0"
git push origin main

git tag v0.2.0
git push origin v0.2.0
```

标签中的版本必须与 `tauri.conf.json` 完全一致，否则工作流会主动失败。构建开始后，可以在仓库的 **Actions** 页面查看进度；完成后，产物会进入 GitHub **Releases** 的草稿版本。请先下载测试 Windows 和 macOS 安装包，确认可以启动服务和连接虚拟局域网，再点击 **Publish release**。

Windows 便携版仍依赖 Microsoft Edge WebView2 Runtime，Windows 10/11 通常已经自带。未配置代码签名时，Windows 和 macOS 可能显示未知发布者或安全提醒。

### 项目文档

- [产品需求文档](docs/prd.md)
- [技术设计文档](docs/trd.md)
- [组网服务产品需求文档](docs/prd-virtual-lan.md)
- [组网服务技术设计文档](docs/trd-docker-server.md)

### 参与贡献

欢迎提交 Issue、功能建议和 Pull Request。

提交代码前请确保：

```bash
npm test -w desktop
npm run build
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
cargo test --manifest-path apps/server/Cargo.toml
npm run build:server-web
```

请保持改动范围清晰，并为重要行为补充测试。安全问题请避免在公开 Issue 中披露可直接利用的细节。

## 开源协议

本项目使用 [MIT License](LICENSE)。你可以自由使用、修改和分发，但软件按“原样”提供，不附带任何形式的担保。
