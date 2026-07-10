# 同网互通 TRD

## 目标

本文档描述“同网互通”第一版的技术实现方案。第一版目标是做出一个稳定的局域网主机中转工具：桌面端一键开启服务，其他设备通过浏览器访问，设备之间以一对一会话的形式聊天和传文件。上传中断即失败并清理临时文件；下载支持 HTTP Range 断点续传。

## 技术栈

- 桌面壳：Tauri 2。
- 桌面前端：React 18、TypeScript、Vite。
- 前端路由：React Router 6，默认使用 Hash Router。
- 前端 UI：Ant Design。
- 前端图标：lucide-react。
- 前端请求：Axios。
- 前端状态：Zustand。
- 前端样式：Less + CSS Modules。
- 后端服务：Tauri Rust 侧内置本地 HTTP 服务。
- 实时通信：WebSocket。
- 本地数据库：SQLite。
- 文件存储：用户配置的保存目录 + 应用数据目录下的临时上传目录。

说明：第一版优先采用 Tauri Rust 内置服务，避免引入 Node sidecar，减少打包复杂度和运行时依赖。浏览器端所有数据通过桌面主机服务中转，不做 P2P。

## 总体架构

```text
Windows / macOS 桌面端
├── Tauri 桌面窗口
│   ├── 服务控制
│   ├── 地址和二维码展示
│   ├── 设备和传输记录
│   └── 设置页
├── Rust 本地服务
│   ├── HTTP 静态资源服务
│   ├── REST API
│   ├── WebSocket 网关
│   ├── 文件上传和下载
│   └── SQLite 持久化
└── 本地文件系统
    ├── 正式保存目录
    └── 临时上传目录

其他设备浏览器
├── Web 客户端页面
├── REST API 调用
└── WebSocket 连接
```

## 运行时模型

桌面端点击“开启互通”后，Rust 侧启动一个监听局域网地址的 HTTP/WebSocket 服务。服务启动时生成本次会话访问令牌，桌面端根据局域网地址和令牌生成访问 URL 与二维码。

浏览器访问时必须携带访问令牌。首次访问会注册或恢复一个浏览器设备身份，用户可以设置设备昵称。所有设备消息、文件事件和在线状态都经过主机服务转发。

第一版的传输语义是“主机中转”：

```text
设备 A 上传文件到主机 -> 主机保存文件 -> 主机给设备 B 发文件消息 -> 设备 B 手动下载
```

用户看到的是设备之间的一对一会话，但底层文件始终先落到主机。

## 前端工程结构

遵循 `react-loose-conventions`，第一版建议结构如下：

```text
src/
├── api/
│   ├── device.ts
│   ├── conversation.ts
│   ├── message.ts
│   ├── file.ts
│   └── service.ts
├── components/
│   ├── DeviceAvatar/
│   ├── FileCard/
│   └── TransferProgress/
├── hooks/
│   ├── useDeviceIdentity.ts
│   ├── useLanSocket.ts
│   └── useUploadTask.ts
├── http/
│   └── index.ts
├── layout/
│   └── AppLayout/
├── router/
│   └── index.tsx
├── store/
│   ├── device.ts
│   ├── conversation.ts
│   ├── transfer.ts
│   └── service.ts
├── styles/
│   ├── variables.less
│   └── mixins.less
├── utils/
│   ├── fileSize.ts
│   ├── storage.ts
│   └── url.ts
├── views/
│   ├── DesktopHome/
│   ├── WebClient/
│   ├── Records/
│   └── Settings/
├── App.tsx
├── main.tsx
├── global.less
└── theme.less
```

不用为了满足结构创建空目录；实现时按功能逐步增加。

## 前端页面

### 桌面端首页

桌面端首页是主机控制台，包含：

- 开启 / 停止服务按钮。
- 当前服务状态。
- 局域网访问地址。
- 二维码。
- 当前访问令牌状态。
- 在线设备数量。
- 最近传输和最近消息摘要。

### 浏览器 Web 客户端

Web 客户端是设备之间聊天和传文件的主要界面，包含：

- 当前设备昵称设置。
- 设备列表。
- 一对一会话窗口。
- 消息列表。
- 文件消息卡片。
- 文件选择和上传入口。
- 下载按钮和下载状态。

### 记录页

记录页用于桌面端查看历史，包含：

- 设备记录。
- 聊天记录。
- 文件记录。
- 成功 / 失败传输记录。

### 设置页

设置页包含：

- 本机设备名。
- 服务端口。
- 文件保存目录。
- 临时文件清理策略。
- 是否每次启动服务生成新令牌。

## Rust 后端模块

建议模块划分：

```text
src-tauri/src/
├── main.rs
├── app_state.rs
├── config.rs
├── server/
│   ├── mod.rs
│   ├── routes.rs
│   ├── auth.rs
│   ├── websocket.rs
│   └── range.rs
├── domain/
│   ├── device.rs
│   ├── conversation.rs
│   ├── message.rs
│   ├── file_record.rs
│   └── transfer.rs
├── storage/
│   ├── mod.rs
│   ├── sqlite.rs
│   └── migrations.rs
├── file_transfer/
│   ├── upload.rs
│   ├── download.rs
│   ├── temp_file.rs
│   └── path_safe.rs
└── tauri_commands/
    ├── service.rs
    ├── settings.rs
    └── records.rs
```

模块职责：

- `server`：HTTP 路由、WebSocket、访问令牌校验、Range 下载。
- `domain`：核心数据结构和状态枚举。
- `storage`：SQLite 初始化、迁移和数据访问。
- `file_transfer`：上传临时文件、最终落盘、失败清理、下载流。
- `tauri_commands`：桌面前端调用的 Tauri 命令。

## 数据模型

### Device

```text
id              设备 ID
name            设备昵称
kind            host / browser
client_id       浏览器本地身份，可为空
last_seen_at    最后在线时间
created_at      创建时间
updated_at      更新时间
```

### Conversation

```text
id              会话 ID
device_a_id     参与设备 A
device_b_id     参与设备 B
created_at      创建时间
updated_at      更新时间
```

一对一会话 ID 由两个设备 ID 规范化生成或在数据库中唯一约束，确保 A-B 和 B-A 是同一个会话。

### Message

```text
id                消息 ID
conversation_id   会话 ID
from_device_id    发送设备
to_device_id      接收设备
type              text / file / system
content           文本内容或系统消息内容
file_id           文件 ID，可为空
created_at        创建时间
```

### FileRecord

```text
id                 文件 ID
conversation_id    所属会话
from_device_id     发送设备
to_device_id       接收设备
original_name      原始文件名
stored_name        落盘文件名
mime_type          MIME 类型
size               文件大小
sha256             文件校验值，可为空
path               本机保存路径
status             available / deleted
created_at         创建时间
```

### TransferRecord

```text
id                传输 ID
kind              upload / download
file_id           文件 ID，可为空
conversation_id   会话 ID
from_device_id    来源设备
to_device_id      目标设备
status            pending / running / success / failed / canceled
bytes_total       总字节数
bytes_done        已完成字节数
error_message     失败原因，可为空
created_at        创建时间
updated_at        更新时间
```

## API 设计

所有浏览器 API 都需要访问令牌。令牌可以通过 URL query 首次进入后保存到浏览器会话状态，后续请求放在 Header 中：

```text
Authorization: Bearer <session_token>
```

### 服务与设备

```text
GET  /api/bootstrap
POST /api/devices/register
PATCH /api/devices/me
GET  /api/devices
```

- `GET /api/bootstrap`：返回服务信息、主机设备、当前浏览器需要的初始化信息。
- `POST /api/devices/register`：注册或恢复浏览器设备。
- `PATCH /api/devices/me`：修改当前设备昵称。
- `GET /api/devices`：获取在线和历史设备列表。

### 会话与消息

```text
GET  /api/conversations
POST /api/conversations/open
GET  /api/conversations/:id/messages
POST /api/conversations/:id/messages
```

- `POST /api/conversations/open`：打开和某个设备的一对一会话，不存在则创建。
- `POST /api/conversations/:id/messages`：发送文本消息。

### 文件上传

```text
POST /api/conversations/:id/files
```

请求使用 `multipart/form-data`。上传过程中服务端写入临时文件；请求完整结束后移动到正式目录、创建文件记录、创建文件消息、广播 WebSocket 事件。

失败策略：

- 客户端断开、中断或服务端写入失败时，上传任务标记为失败。
- 删除该上传对应的临时文件。
- 不创建可下载的正式文件记录。
- 可创建失败传输记录，用于桌面端历史查看。

### 文件下载

```text
GET /api/files/:id/download
```

下载接口支持 HTTP Range：

```text
Range: bytes=1048576-
```

服务端返回：

```text
206 Partial Content
Content-Range: bytes 1048576-9999999/10000000
Accept-Ranges: bytes
```

没有 Range 时返回完整文件。非法 Range 返回 `416 Range Not Satisfiable`。

### WebSocket

```text
GET /ws?token=<session_token>&deviceId=<device_id>
```

主要事件：

```text
device_online
device_offline
message_created
file_message_created
transfer_progress
transfer_finished
transfer_failed
service_stopping
```

WebSocket 只负责实时通知，不作为唯一数据源。页面刷新后应通过 REST API 拉取最新状态。

## 关键流程

### 开启服务

1. 桌面端调用 Tauri 命令启动服务。
2. 后端读取设置，选择监听端口。
3. 获取可访问的局域网 IP。
4. 生成本次服务访问令牌。
5. 创建或更新主机设备记录。
6. 启动 HTTP/WebSocket 服务。
7. 返回访问地址和二维码内容给桌面端。

### 浏览器加入

1. 浏览器打开带令牌的访问 URL。
2. Web 客户端读取 URL token。
3. 调用初始化 API。
4. 从本地存储读取已有 `clientId`；没有则生成。
5. 调用设备注册接口。
6. 建立 WebSocket 连接。
7. 拉取设备列表和会话列表。

### 发送文本消息

1. 用户打开目标设备会话。
2. 前端调用发送消息 API。
3. 后端校验会话参与者。
4. 写入消息记录。
5. 通过 WebSocket 通知发送方和接收方。

### 上传文件

1. 用户在会话中选择文件。
2. 前端创建上传任务并显示进度。
3. 使用 `multipart/form-data` 上传。
4. 后端创建临时文件并流式写入。
5. 上传完整后原子移动到正式目录。
6. 后端创建文件记录、传输记录和文件消息。
7. 后端通知会话双方。
8. 如果任一步失败，删除临时文件并记录失败。

### 下载文件

1. 用户点击文件卡片下载。
2. 浏览器请求下载接口。
3. 如果浏览器或下载器带 Range，后端返回指定字节范围。
4. 下载完成后可记录下载传输状态。

## 存储与文件规则

- 正式文件目录由用户配置。
- 临时上传目录使用应用数据目录，不和正式目录混放。
- 上传成功前不在文件列表中展示为可下载文件。
- 正式落盘文件名需要防止路径穿越和同名冲突。
- 原始文件名只作为展示字段保存。
- 失败上传的临时文件应立即删除；启动服务时也应扫描并清理过期临时文件。

## 安全策略

- 默认只面向可信局域网使用。
- 每次开启服务默认生成新的访问令牌。
- 所有浏览器 API 和 WebSocket 都需要令牌。
- 文件下载接口需要校验当前设备是否是会话参与者。
- 上传文件名不能直接拼接进保存路径。
- 禁止通过 API 访问保存目录以外的任意路径。
- 桌面端应清楚展示当前服务正在局域网开放。

## 前端状态分层

- `service` store：服务状态、访问地址、二维码、令牌状态。
- `device` store：当前设备、主机设备、在线设备列表。
- `conversation` store：会话列表、当前会话、消息缓存。
- `transfer` store：上传任务、下载任务、进度和失败状态。

页面局部状态保留在页面内，例如输入框内容、弹窗开关、当前选择文件等。跨页面或 WebSocket 事件需要更新的数据放入 Zustand。

## 依赖建议

默认依赖：

```text
antd
lucide-react
axios
zustand
lodash
react-router-dom
less
```

第一版建议额外使用：

```text
qrcode
```

用于桌面端生成访问二维码。如果实现时选择后端生成二维码，也可以换成 Rust 侧二维码库。

暂不默认引入：

- `dayjs`：如果只展示少量时间，可以先用原生 `Intl.DateTimeFormat`。
- `ahooks`：等出现足够多通用 hook 场景再引入。
- `zod`：如果前后端类型共享不足或需要运行时校验，再引入。
- `file-saver`：第一版下载直接使用浏览器链接即可。

## 测试方案

### 后端测试

- 访问令牌校验。
- 设备注册和昵称更新。
- 一对一会话创建去重。
- 文本消息只投递给会话双方。
- 文件上传成功后生成文件记录和文件消息。
- 文件上传失败后清理临时文件。
- 文件名安全处理。
- HTTP Range 下载，包括完整下载、部分下载、非法 Range。

### 前端测试

- 桌面端服务状态展示。
- 二维码和访问地址展示。
- 浏览器设备注册和昵称修改。
- 设备列表在线 / 离线状态。
- 一对一会话切换。
- 文本消息发送和接收。
- 文件上传成功、失败和进度展示。
- 文件卡片下载入口。

### 端到端测试

用浏览器自动化覆盖最小闭环：

1. 启动桌面服务。
2. 打开两个浏览器客户端。
3. 分别设置昵称。
4. A 给 B 发文字消息。
5. A 给 B 上传文件。
6. B 看到文件消息并下载。
7. 使用 Range 请求验证下载断点续传。
8. 停止服务后，新请求无法访问。

## 里程碑

### M1：项目骨架

- 初始化 Tauri + React + TypeScript + Vite 项目。
- 配置 `@/` 路径别名。
- 配置 Hash Router。
- 接入 Ant Design、lucide-react、Less、Zustand、Axios。
- 建立桌面端首页和浏览器客户端基础路由。

### M2：服务生命周期

- Rust 侧启动 / 停止 HTTP 服务。
- 桌面端展示运行状态、访问地址和二维码。
- 生成访问令牌。
- 浏览器能打开 Web 客户端。

### M3：设备和会话

- 浏览器设备注册。
- 设备昵称。
- 在线状态。
- 一对一会话创建。
- WebSocket 连接和事件广播。

### M4：聊天

- 文本消息 API。
- 消息持久化。
- 会话消息列表。
- 实时消息投递。

### M5：文件传输

- 文件上传到临时目录。
- 上传成功后移动到正式目录。
- 上传失败清理临时文件。
- 文件消息。
- 文件下载。
- HTTP Range 断点续传。

### M6：记录、设置和打磨

- 本地记录页。
- 保存目录和端口设置。
- 服务启动时清理过期临时文件。
- 错误提示和空状态。
- 核心流程端到端测试。

## 暂不实现

- 上传断点续传。
- P2P 直连。
- 群聊。
- 公网访问。
- 用户账号。
- 云同步。
- 端到端加密。
- 手机原生 App。
- 复杂共享目录浏览。
