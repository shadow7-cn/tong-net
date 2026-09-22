# 界面截图

[返回首页](../README.md) · [下载安装](https://github.com/shadow7-cn/tong-net/releases/latest) · [部署组网服务](deploy-virtual-lan.md)

以下图片来自 v0.2.1 当前前端界面的浏览器截图。为保护隐私，使用独立的演示接口数据；设备名称、聊天内容、文件、地址、在线状态和流量均为示例，不代表真实网络连接测试。二维码和令牌也仅用于演示，请使用自己应用中生成的二维码。

桌面截图不包含操作系统窗口边框；手机截图为 390px 宽的移动端视口预览。

## 导航

- [桌面端](#桌面端)：互通服务、群聊、群成员、虚拟局域网、设置。
- [互通记录](#互通记录)：访问端、聊天、文件、传输。
- [手机 Web 端](#手机-web-端)：群聊和群成员。
- [Linux 组网服务管理端](#linux-组网服务管理端)：首次设置、登录、概览、网络、设备、审计、设置。

## 桌面端

### 互通服务

一键开启互通，查看在线访问端，并通过局域网或虚拟局域网二维码邀请其他设备。

![互通服务与扫码入口](images/desktop-service.png)

### 互通群聊

所有访问端进入同一个群，实时聊天、共享文件；桌面端可复制接入地址，也可将文件另存到指定位置。

![桌面端互通群聊](images/desktop-chat.png)

### 群成员

默认只显示在线成员，可以修改自己的名称，或切换到全部成员查看离线访问端。

![桌面端群成员列表](images/desktop-members.png)

### 虚拟局域网

填写自建服务地址和网络信息，查看虚拟 IP、在线设备、延迟、连接协议与流量。

![虚拟局域网连接与在线设备](images/desktop-virtual-lan.png)

### 设置

配置启动时自动开启互通、主机名称、服务端口、文件保存目录和访问令牌策略。

![桌面端设置](images/desktop-settings.png)

## 互通记录

记录保存在主机本地，按访问端、聊天、文件和传输分类查看。

### 访问端记录

![访问端记录](images/records-devices.png)

### 聊天记录

![聊天记录](images/records-messages.png)

### 文件记录

![文件记录](images/records-files.png)

### 传输记录

![传输记录](images/records-transfers.png)

## 手机 Web 端

手机无需安装客户端，扫码后直接进入群聊。文件卡片提供下载与复制链接入口，群成员面板可修改自己的名称。

<p>
  <img src="images/mobile-chat.png" width="320" alt="手机 Web 端群聊与文件下载" />
  <img src="images/mobile-members.png" width="320" alt="手机 Web 端在线群成员" />
</p>

## Linux 组网服务管理端

这是自建 Docker 服务提供的管理页面，与桌面端互通服务分别运行。部署步骤见[组网服务部署教程](deploy-virtual-lan.md)。

### 首次设置

创建管理员，设置站点信息，选择公共节点或私有节点模式。

![组网服务首次设置](images/server-setup.png)

### 管理员登录

![组网服务登录](images/server-login.png)

### 运行概览

查看节点健康状态、网络数量、设备数量和当前在线数量。

![组网服务运行概览](images/server-overview.png)

### 私有网络

创建网络、刷新列表、停用网络或重置网络密码。

![私有网络管理](images/server-networks.png)

### 设备管理

按网络查看设备、在线状态与流量，添加备注或撤销设备凭据。

![组网设备管理](images/server-devices.png)

### 审计日志

![组网服务审计日志](images/server-audit.png)

### 服务端设置

管理站点与管理员信息、切换节点模式，并查看 Docker 配置的运行端口。

![组网服务设置](images/server-settings.png)

[返回首页](../README.md)
