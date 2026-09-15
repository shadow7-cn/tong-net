# 部署虚拟局域网服务

本教程帮助你在 Linux 服务器上部署「同网互通组网服务」，让不同网络下的电脑通过桌面端加入同一个虚拟局域网。

服务器提供组网接入和 Web 管理页面，不是聊天或文件存储服务器。聊天、文件和互通记录仍由开启互通服务的电脑保存；使用群聊时，这台电脑需要保持运行。

当前采用**从源码构建 Docker 镜像**的方式。暂不提供镜像市场安装，也不需要在服务器上安装桌面端、Node.js 或 Rust，构建工具都在 Docker 内运行。

## 1. 准备服务器

- Linux 服务器，支持 x86_64 或 ARM64。
- 已安装 Git、Docker Engine 和 Docker Compose 插件，当前用户有运行 Docker 的权限。
- 存在 `/dev/net/tun`，容器可以使用虚拟网卡。
- 构建时能够访问 npm、Rust 包仓库、Debian 软件源和 GitHub，能够拉取 Docker 基础镜像。

在服务器终端执行以下命令检查：

```bash
git --version
docker version
docker compose version
ls -l /dev/net/tun
```

如果没有安装 Docker，请按 [Docker 官方 Linux 安装说明](https://docs.docker.com/engine/install/)选择对应发行版。若缺少 TUN 设备，请联系服务器提供商确认虚拟化环境是否支持，不要直接移除 Compose 中的设备配置。

### 放通端口

同时检查云服务器安全组和系统防火墙：

| 默认端口 | 协议 | 用途 |
| --- | --- | --- |
| 17280 | TCP | Web 管理页面、桌面端获取组网配置及认证 |
| 11010 | TCP 和 UDP | EasyTier 组网连接 |

两个端口用途不同，都需要考虑。自定义端口后，应放通修改后的端口。管理端口可按需限制来源，或通过 HTTPS 反向代理提供访问。

> Compose 会向容器授予 `NET_ADMIN`、`NET_RAW` 并挂载 TUN 设备，这是组网功能所需的网络权限；无需改成 `privileged` 模式。

## 2. 获取代码和配置

以下命令均在服务器终端执行。后续命令默认在 `tong-net` 项目根目录运行。

```bash
git clone https://github.com/shadow7-cn/tong-net.git
cd tong-net
cp deploy/.env.example deploy/.env
```

打开 `deploy/.env`，默认配置如下：

```dotenv
TONGNET_WEB_PORT=17280
TONGNET_EASYTIER_PORT=11010
TZ=Asia/Shanghai
RUST_LOG=info
```

没有端口冲突就无需修改。例如，想将管理端口改为 `18080`，将组网端口改为 `12010`，只需修改对应两个值。Compose 会同步调整容器监听端口和宿主机映射，不要只修改映射的一端。

> 更新已有部署时，不要再次用示例文件覆盖自己的 `.env`。

## 3. 构建并启动

```bash
docker compose --env-file deploy/.env -f deploy/docker-compose.yml build
docker compose --env-file deploy/.env -f deploy/docker-compose.yml up -d
docker compose --env-file deploy/.env -f deploy/docker-compose.yml ps
```

首次构建需要下载依赖并编译，耗时取决于服务器性能和网络。看到容器启动后，可继续查看日志；健康检查可能需要稍等一会儿。

```bash
docker compose --env-file deploy/.env -f deploy/docker-compose.yml logs --tail=100 -f
```

按 `Ctrl+C` 只退出日志查看，不会停止容器。

浏览器打开以下地址，将“服务器IP”替换成你的实际地址；修改过管理端口时也要替换端口：

```text
http://服务器IP:17280
```

> **公网正式使用建议配置 HTTPS。** HTTP 会明文传输管理登录及网络认证信息，桌面端也会提示风险。可以使用 Caddy、Nginx 或 1Panel 将 HTTPS 域名反向代理到管理端口；不要将该端口暴露为无保护的公网登录入口。EasyTier 的 TCP/UDP 组网端口仍需单独放通，不能仅靠 HTTP 反向代理。

## 4. 首次设置

首次进入管理页面，填写：

| 字段 | 填写内容 |
| --- | --- |
| 管理员用户名、管理员密码 | 自己设置，用来登录 Web 管理页面，妥善保管 |
| 站点名称 | 自己给服务起的名字 |
| 对外 IP 或域名 | 客户端能访问的服务器公网 IP 或域名，例如 `vpn.example.com`，不带 `http://`、端口或路径 |
| 节点模式 | 私有节点或公共节点 |

密码输入旁的随机按钮可以生成密码。管理员密码和网络密码不是同一个用途，不要把管理员密码发给普通组网用户。

### 私有节点：适合自己和固定成员

首次设置时创建第一个网络，填写网络名称与网络密码；之后可在「网络」页管理其他网络。

将**服务端地址、网络名称、网络密码**告诉需要加入的人。服务端验证网络信息并签发设备凭据，管理员可以查看设备并按网络撤销设备。不同网络拥有各自的认证和撤销范围。

### 公共节点：让使用者自行组网

服务端提供共享节点，不保存使用者填写的网络名称和网络密码。需要互联的人，在各自桌面端填写相同的网络名称和网络密码；使用不同组合可组成不同网络。

公共模式不是“所有人自动加入一个群”，管理页面里的成员信息也不等同于私有模式下完整的网络管理。公开提供节点前，请考虑服务器带宽消耗和使用范围。

## 5. 在同网互通桌面端连接

打开桌面端的「虚拟局域网」，填写：

| 桌面端字段 | 示例与说明 |
| --- | --- |
| 组网服务端地址 | `https://vpn.example.com`；测试 HTTP 时为 `http://服务器IP:17280`。这里填管理地址，不是 `tcp://...:11010` |
| 网络名称 | 私有模式填管理员创建的名称；公共模式由同组成员约定 |
| 网络密码 | 对应的网络密码，不是 Web 管理员密码 |
| 本机设备名称 | 自己设置，用来在成员列表辨认电脑 |

点击连接。首次使用可能需要按系统提示安装或授权网络服务。

## 6. 确认连接成功

1. 桌面端状态显示「已连接」，并出现虚拟 IP。
2. 让另一台电脑用相同网络信息连接，检查成员列表是否出现对方。
3. 在其中一台电脑开启「互通服务」，通过虚拟局域网二维码或群聊顶部的虚拟地址让其他设备加入。

扫描虚拟局域网二维码的设备也必须能访问该虚拟网络；二维码本身不会给手机安装 EasyTier，也不会自动完成手机组网。同一物理局域网内的手机可使用普通局域网二维码。

建议在桌面端设置中关闭「允许无令牌访问」，并通过带令牌的完整链接或二维码邀请成员。组网连接成功不等于互通服务已开启，这两项功能相互独立。

## 7. 日常维护

### 停止与重新启动

```bash
docker compose --env-file deploy/.env -f deploy/docker-compose.yml down
docker compose --env-file deploy/.env -f deploy/docker-compose.yml up -d
```

停止组网服务会影响连接。数据在宿主机的 `deploy/data` 中，以上停止命令不会删除该目录。

### 更新

先备份，再获取代码并重新构建。更新期间组网可能暂时中断，建议提前通知使用者。

```bash
git pull --ff-only
docker compose --env-file deploy/.env -f deploy/docker-compose.yml up -d --build
docker compose --env-file deploy/.env -f deploy/docker-compose.yml ps
```

### 备份与恢复

备份目录包含数据库、加密密钥和设备凭据，请当作敏感文件保存，不要提交到 GitHub。

```bash
docker compose --env-file deploy/.env -f deploy/docker-compose.yml down
tar -czf "tong-net-backup-$(date +%Y%m%d-%H%M%S).tar.gz" deploy/data deploy/.env
docker compose --env-file deploy/.env -f deploy/docker-compose.yml up -d
```

备份命令需要读取 `deploy/data` 的权限。若提示权限不足，应使用有权限的管理员操作，不要将数据目录改为所有人可读写。

恢复时先停止服务、备份现有目录，再将归档内的 `deploy/data` 和 `deploy/.env` 完整恢复到项目目录，保留文件权限后启动。**不能只恢复 SQLite 数据库而遗漏密钥**，否则加密配置无法解密。更换公网地址后，还需检查管理页面中的对外 IP 或域名。

### 忘记管理员密码

在容器运行时执行：

```bash
docker exec -it tong-net-server tong-net-server admin reset-password
```

根据提示输入两次新密码。已有管理登录会失效，需要重新登录。这是重置管理员密码，不是修改组网使用的网络密码。

## 8. 常见问题

| 问题 | 优先检查 |
| --- | --- |
| 镜像构建失败 | 查看失败步骤；检查服务器内存、磁盘和依赖下载网络，不要跳过 EasyTier 文件校验 |
| 管理页面打不开 | 容器是否健康；管理端口映射、安全组、防火墙及访问地址是否正确 |
| 提示端口被占用 | 修改 `deploy/.env` 后重新执行启动命令，并同步修改防火墙与客户端地址 |
| 提示 TUN 或网络权限错误 | 宿主机是否有 `/dev/net/tun`，是否保留 Compose 中的设备挂载与网络权限 |
| 页面能打开，但组网连接失败 | 对外 IP/域名是否正确；组网端口是否同时放通 TCP 和 UDP；私有网络是否启用，名称与密码是否正确 |
| 已连接但看不到其他电脑 | 对方是否在线、是否使用相同网络信息；成员列表不显示服务用途的内部节点 |
| 有虚拟 IP，但群聊网页打不开 | 主机是否开启互通服务；主机防火墙是否允许互通端口；访问者是否可达该虚拟 IP；是否使用完整邀请链接 |
| 手机扫码无法进入虚拟网络 | 手机是否已接入相应虚拟网络；只有扫码并不能自动组网 |

排查时可查看日志，但公开提交日志前请移除密码、令牌、设备凭据及不希望公开的地址。

---

[返回项目首页](../README.md)
