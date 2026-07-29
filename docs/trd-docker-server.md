# 同网互通组网服务 TRD

> 对应 PRD：`docs/prd-virtual-lan.md`
> 版本：0.2.0

## 1. 架构

```text
管理员浏览器
    │ HTTP/HTTPS
    ▼
Rust 管理服务（PID 1）
├── 内嵌 React 管理 Web
├── REST API
├── SQLite
├── EasyTier 共享节点进程
└── 私有网络管理实例（最多 10）
        │ localhost RPC / easytier-cli
        ▼
Windows / macOS 同网互通 EasyTier Core
```

单容器包含：

- `tong-net-server`
- 管理 Web 静态资源
- `easytier-core`
- `easytier-cli`
- SQLite 数据库
- EasyTier 第三方许可证

## 2. 工程结构

```text
apps/
├── desktop/
├── server/
│   ├── Cargo.toml
│   ├── tests/
│   └── src/
└── server-web/
    ├── package.json
    └── src/

docker/
└── server.Dockerfile

deploy/
├── docker-compose.yml
└── .env.example

scripts/
└── prepare-easytier-linux.mjs
```

服务端：Rust、Axum、Tokio、rusqlite、Argon2id、AES-256-GCM、Tracing。

管理 Web：React、TypeScript、Vite、Hash Router、Ant Design、Lucide、Less。

## 3. 运行进程

### 3.1 Rust PID 1

职责：

1. 读取环境变量和 `/data`。
2. 执行数据库迁移。
3. 启动 Web/API。
4. 根据当前模式生成 EasyTier 配置。
5. 启动、停止和监控子进程。
6. 定期读取 EasyTier RPC。
7. 处理 `SIGTERM`/`SIGINT`，停止全部 Core 后退出。

不使用 systemd、Supervisor 或 Docker Socket。

### 3.2 共享节点

共享节点统一监听 EasyTier 配置端口。

公共模式：

```text
--network-name <random-shared-network>
--local-private-key <persisted-key>
--secure-mode
--relay-all-peer-rpc
--listeners tcp://0.0.0.0:<port>
--listeners udp://0.0.0.0:<port>
--rpc-portal 127.0.0.1:15888
```

不设置中继网络白名单，允许外部网络使用共享节点。

私有模式额外设置：

```text
--relay-network-whitelist <internal-network-1> <internal-network-2> ...
```

白名单只包含启用的私有网络内部名称。更新网络后受控重启共享节点。

### 3.3 私有网络管理实例

每个启用的私有网络运行一个独立 EasyTier 实例：

```text
--instance-name private-<network-id>
--network-name <internal-network-name>
--network-secret <internal-network-secret>
--secure-mode
--local-private-key <network-private-key>
--credential-file /data/easytier/networks/<id>/credentials.json
--no-listener
--peers tcp://127.0.0.1:<shared-port>
--rpc-portal 127.0.0.1:<allocated-rpc-port>
```

RPC 端口从 `15900` 起按网络槽位分配，不对公网开放。实例只用于签发/撤销凭据和传播信任，不创建额外公网 Listener。

### 3.4 重启策略

- 正常停止不重启。
- 异常退出按 1、2、4、8、16、30 秒退避。
- 子进程恢复正常后清零退避。
- 重试间隔最高 30 秒，服务退出时先停止监控再清理全部子进程。
- 管理员可在 UI 手动重试。

## 4. 配置

| 环境变量 | 默认值 | 说明 |
| --- | --- | --- |
| `TONGNET_WEB_PORT` | `17280` | Web/API 实际监听端口 |
| `TONGNET_EASYTIER_PORT` | `11010` | EasyTier TCP/UDP 端口 |
| `TONGNET_DATA_DIR` | `/data` | 持久化目录 |
| `TONGNET_LOG_LEVEL` | `info` | 日志级别 |
| `TZ` | `Asia/Shanghai` | 时区 |

端口只通过环境变量修改，Web 设置页只读展示。

## 5. 持久化

```text
/data/
├── db/tong-net-server.sqlite3
├── keys/master.key
├── easytier/
│   ├── shared/
│   │   ├── private.key
│   │   └── config.toml
│   └── networks/<network-id>/
│       ├── private.key
│       ├── credentials.json
│       └── config.toml
└── logs/
```

敏感文件权限 `0600`，目录 `0700`。所有可恢复数据必须位于 `/data`。

## 6. 数据模型

### 6.1 `site_settings`

- `id = 1`
- `initialized`
- `site_name`
- `public_host`
- `mode`: `public | private`
- `created_at`
- `updated_at`

### 6.2 `admin_users`

第一版最多一条：

- `id`
- `username`（唯一）
- `password_hash`
- `session_generation`
- `last_login_at`
- `created_at`
- `updated_at`

### 6.3 `admin_sessions`

- `id`
- `token_hash`
- `generation`
- `expires_at`
- `created_at`
- `last_used_at`

管理员 Cookie 使用随机不透明令牌，数据库只存 SHA-256 哈希。有效期 12 小时，HttpOnly、SameSite=Lax；HTTPS 下设置 Secure。

### 6.4 `private_networks`

- `id`
- `name`
- `name_normalized`（唯一）
- `password_hash`
- `internal_name_ciphertext`
- `internal_secret_ciphertext`
- `private_key_ciphertext`
- `status`: `active | disabled`
- `slot`：0 至 9
- `created_at`
- `updated_at`

私有网络公开密码只保存 Argon2id 哈希。EasyTier 内部秘密使用站点主密钥 AES-256-GCM 加密。

### 6.5 `devices`

站点级稳定设备：

- `id`
- `client_device_id`（唯一）
- `name`
- `platform`
- `client_version`
- `created_at`
- `updated_at`

### 6.6 `network_memberships`

- `id`
- `network_id`
- `device_id`
- `admin_note`
- `status`: `active | revoked`
- `credential_id`
- `credential_secret_ciphertext`
- `virtual_ip`
- `protocol`
- `latency_ms`
- `rx_bytes`
- `tx_bytes`
- `last_seen_at`
- `created_at`
- `updated_at`

唯一约束 `(network_id, device_id)`。撤销只影响该成员关系。

### 6.7 `device_sessions`

- `id`
- `membership_id`
- `token_hash`
- `expires_at`
- `revoked_at`
- `created_at`
- `last_seen_at`

每次手动连接使用网络密码登录并创建单层设备会话。正常断开撤销会话但保留 EasyTier 凭据。

### 6.8 `audit_logs`

- `id`
- `actor_type`
- `actor_id`
- `action`
- `target_type`
- `target_id`
- `result`
- `ip_address`
- `metadata_json`
- `created_at`

每日清理超过 90 天或总数超过 10,000 的旧记录。

## 7. 认证

### 7.1 管理员

- 初始化时创建用户名和至少 8 位密码。
- 密码使用 Argon2id。
- 登录失败按 IP 限速。
- 管理危险操作要求再次提交管理员密码。
- 用户名和密码修改后 `session_generation + 1`，旧会话失效。
- 忘记密码通过容器本地 CLI 重置。

### 7.2 公共网络

桌面端只调用公开信息接口。网络名称和密码直接进入本机 EasyTier Core，不进入 Linux HTTP API。

### 7.3 私有网络

桌面端提交：

- 网络名称
- 网络密码
- 稳定设备 ID
- 设备名称
- 平台
- 客户端版本

服务验证网络密码后：

1. 创建或恢复设备和成员关系。
2. 已撤销成员拒绝连接。
3. 本地凭据缺失时撤销旧凭据并重新签发。
4. 创建单层设备会话。
5. 返回内部网络名称、EasyTier 凭据、共享节点 TCP/UDP 地址和固定公钥。

网络主密码永不下发。

## 8. API

前缀 `/api/v1`。错误格式：

```json
{
  "code": "NETWORK_PASSWORD_INVALID",
  "message": "网络名称或密码错误",
  "requestId": "..."
}
```

### 8.1 公开

- `GET /api/v1/info`
  - 初始化状态、站点名称、模式、版本、EasyTier 端口、公共主机、共享节点公钥。
- `POST /api/v1/setup`
  - 仅未初始化时可用；事务创建管理员、站点和可选首个私有网络。
- `GET /healthz`

### 8.2 管理员

- `POST /api/v1/admin/login`
- `POST /api/v1/admin/logout`
- `GET /api/v1/admin/overview`
- `GET /api/v1/admin/networks`
- `POST /api/v1/admin/networks`
- `POST /api/v1/admin/networks/:id/disable`
- `POST /api/v1/admin/networks/:id/enable`
- `POST /api/v1/admin/networks/:id/reset-password`
- `DELETE /api/v1/admin/networks/:id`
- `GET /api/v1/admin/devices`
- `PATCH /api/v1/admin/memberships/:id`
- `POST /api/v1/admin/memberships/:id/revoke`
- `DELETE /api/v1/admin/memberships/:id`
- `GET /api/v1/admin/audit-logs`
- `DELETE /api/v1/admin/audit-logs`
- `GET /api/v1/admin/settings`
- `PATCH /api/v1/admin/settings`
- `POST /api/v1/admin/mode`
- `POST /api/v1/admin/easytier/retry`

模式切换保留私有网络数据，停止所有 Core 后按新模式重建。

### 8.3 私有设备

- `POST /api/v1/private/connect`
- `POST /api/v1/private/heartbeat`
- `POST /api/v1/private/disconnect`
- `PATCH /api/v1/private/device`

心跳每 10 秒，30 秒无心跳视为离线。设备改名限制 1 至 40 个 Unicode 字符且拒绝控制字符。

## 9. EasyTier 适配

```rust
trait EasyTierSupervisor {
    async fn apply_mode(&self, snapshot: RuntimeSnapshot) -> Result<()>;
    async fn status(&self) -> Result<ServiceStatus>;
    async fn issue_credential(&self, network_id: &str, device_id: &str) -> Result<Credential>;
    async fn revoke_credential(&self, network_id: &str, credential_id: &str) -> Result<()>;
    async fn shutdown(&self) -> Result<()>;
}
```

凭据：

```text
easytier-cli -p <rpc> -o json credential generate
  --ttl 31536000
  --credential-id <stable-uuid>
  --reusable false
```

撤销：

```text
easytier-cli -p <rpc> -o json credential revoke <credential-id>
```

固定 EasyTier 版本 2.6.4。所有 CLI JSON 输出保存脱敏测试样本并覆盖解析测试。业务逻辑不得通过日志猜测连接状态。

公共模式共享节点不设置 `relay-network-whitelist`；私有模式只允许启用网络的隐藏内部名称。

## 10. 桌面端改造

`EasyTierConfig` 改为：

```ts
interface EasyTierConfig {
  serverUrl: string;
  networkName: string;
  networkPassword: string;
  deviceName: string;
}
```

启动前：

1. 调用 `/api/v1/info`。
2. 检查服务已初始化和版本兼容。
3. HTTP 地址首次连接显示确认。
4. 公共模式直接组合本地 Core 参数。
5. 私有模式调用 `/private/connect` 获取凭据。

公共模式 Core：

```text
--network-name <user-name>
--network-secret <user-password>
--secure-mode
--peers tcp://<public-host>:<port>
--peers udp://<public-host>:<port>
公共模式桌面端和服务端内部管理节点通过 EasyTier TOML 的
`[[peer]].peer_public_key` 固定共享节点身份公钥。EasyTier 2.6.4 会拒绝尚未取得
业务网可信列表、仅持临时凭据的节点直接固定共享节点公钥，因此私有模式桌面端
先建立加密共享节点通道，再由业务网管理节点验证临时凭据；升级 EasyTier 时应
复核该限制是否已经解除。
```

私有模式 Core：

```text
--network-name <internal-name>
--credential <device-credential>
--peers tcp://<public-host>:<port>
--peers udp://<public-host>:<port>
```

桌面端保存最近配置，网络密码使用现有 AES-256-GCM 加密。私有成员凭据由服务端加密保存，桌面端每次手动连接时重新获取，不落盘。正常断开私有模式时调用 `/private/disconnect`，不撤销 EasyTier 凭据。

## 11. 管理 Web

路由：

- `#/setup`
- `#/login`
- `#/overview`
- `#/networks`
- `#/devices`
- `#/audit`
- `#/settings`

复用桌面端设计语言：

- 同一 Logo。
- 左侧导航和青绿色主色。
- Ant Design 表格、Tag、表单和确认框。
- 不嵌套卡片，不使用营销 Hero。
- 页面内容区域独立滚动。
- 手机端抽屉导航和列表布局。

随机密码按钮在浏览器使用 `crypto.getRandomValues` 生成 20 位大小写字母和数字。

## 12. Docker

多阶段 Dockerfile：

1. Node 20 构建管理 Web。
2. Rust stable 构建服务。
3. 下载 EasyTier 2.6.4 Linux AMD64/ARM64 并校验 SHA-256。
4. 最终镜像只包含运行二进制、CA 和许可证。

Compose：

```yaml
services:
  tong-net-server:
    build:
      context: ..
      dockerfile: docker/server.Dockerfile
    container_name: tong-net-server
    restart: unless-stopped
    network_mode: host
    cap_add:
      - NET_ADMIN
      - NET_RAW
    devices:
      - /dev/net/tun:/dev/net/tun
    environment:
      TONGNET_WEB_PORT: ${TONGNET_WEB_PORT:-17280}
      TONGNET_EASYTIER_PORT: ${TONGNET_EASYTIER_PORT:-11010}
      TZ: ${TZ:-Asia/Shanghai}
    volumes:
      - ./data:/data
```

## 13. 健康与日志

`/healthz` 返回：

- 初始化状态。
- 数据库状态。
- `/data` 状态。
- 共享节点进程/RPC。
- 私有网络实例总数、正常数和故障数。
- 版本。

Rust 和 EasyTier 日志输出到 stdout/stderr，结构化脱敏。Docker 负责日志轮转。

## 14. 测试

### 14.1 本地

- Rust 单元和 API 集成测试。
- Argon2id、AES-GCM、会话、网络上限、成员撤销、模式切换。
- EasyTier 命令生成和 JSON 解析。
- React 组件与请求测试。
- 桌面端现有全部回归测试。
- Cargo fmt、Clippy、测试、Release 构建。

### 14.2 Docker

- 从源码构建。
- 无 TUN、无权限、端口占用的明确错误。
- 初始化、重启、删除重建、数据恢复。
- 公共/私有切换。
- 自定义端口。
- Core 崩溃和退出清理。

### 14.3 真实服务器

服务器 `43.162.103.53`：

- Ubuntu 22.04 x86_64。
- Web 默认 17280/TCP。
- EasyTier 默认 11010/TCP+UDP。
- 必要时增加 Swap 完成源码构建。

验收公共模式、私有多网络、凭据撤销、密码重置、心跳、虚拟 IP、聊天、文件上传下载和 Range。

Mac 做真实桌面端；Linux 隔离节点模拟第二设备。Windows完成构建验证，真机后补。

### 14.4 清洁交付

测试完成后：

1. 保存脱敏测试报告。
2. 删除测试数据库、密钥、网络和凭据。
3. 重建未初始化 `/data`。
4. 服务保持运行在首次初始化页面。
