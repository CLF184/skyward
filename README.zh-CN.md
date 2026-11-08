![Skyward Recovery](assets/hero.jpg)

# Skyward for Xiaomi 15 Pro

[English](README.md) | 简体中文

Skyward 是使用 Rust 从零编写的 recovery。本仓库是为**一台手机**定制的分支：
**小米 15 Pro（`haotian`）**，基于中国版系统 OS4.0.0.19。

<p align="center">
  <img src="assets/home.png" width="30%" alt="首页">
  <img src="assets/settings-dark.png" width="30%" alt="深色模式下的设置">
  <img src="assets/time-zone.png" width="30%" alt="时区">
</p>

## 本分支相对上游补了什么

上游的 Skyward 是多机型的 recovery。本分支只保留 `devices/haotian.toml`，并补上
解密这台手机 data 分区实际需要的东西：

- **weaver 能应答了，所以只绑定 weaver 的用户也能解开。** 这类用户的密钥由安全元件
  里的 weaver 看守，而 recovery 默认不会启动它依赖的三样东西，现在都由 Skyward 负责：
  - `ssgtzd`，TEE 的 mink 服务。没有它，安全元件的 `TEEC_OpenSession` 一律返回
    `0xFFFF0000`，安全元件根本打不开。
  - `stm_st54se_gpio` 与 `stm_nfc_i2c` 两个内核模块，否则 `/dev/st54spi_gpio`
    不存在。
  - 安全元件的 HAL 以 `root` 运行，并且在 ADSP 就绪之后才启动。
- **双击亮屏**，与手机自带系统一致。开启后息屏时不进入 suspend——因为睡下去就收不到
  触摸了。
- **ADSP 只等 3 秒，等不到就请加载器重试一次。** 某些版本的 `/vendor/firmware`
  里 DSP 固件只有前半段，内核读到第一个缺失分段就停；Skyward 在 modem 分区被显示到
  `/firmware`（内核固件搜索路径的最后一站）之后，请加载器再试一次。

## 功能

### 已实现

- 使用锁屏密码解密 data 分区，包括密钥只绑定 weaver、没有 `secdis` 备份的用户
- 安装刷机包、系统更新包和分区镜像，支持内部存储、U 盘和 adb sideload
- 刷机包的安装脚本可以使用 `7za` 和 `zstd`；`unzip -p` 边解压边输出镜像
- 刷机包的安装脚本可以使用 `avbctl` 关闭验证启动的校验，使用 `resetprop` 读写属性
- 在线系统更新
- Virtual A/B 更新：读取更新快照的状态，把分区映射到快照之上，再把快照合并回去
- 从[本仓库](https://github.com/Rinrroy/skyward-for-haotian/releases)的最新版本在线更新 Skyward
- 备份和恢复
- 清除数据、恢复出厂设置
- 挂载分区，修复和格式化文件系统
- 文件管理和终端
- 连接 WLAN
- MTP 文件传输、adb 和 fastbootd
- 重启到系统、recovery、fastboot、bootloader 或 EDL，以及切换槽位
- 双系统：在首页的两张卡片上选择，滑动切换
- 浅色与深色主题；简体中文、繁体中文和英文

### 未实现

- 由 `snapuserd` 提供压缩快照的更新：本 recovery 只映射并合并内核快照
- 增量更新包
- 没有 A/B 槽位的手机的更新包

## 支持的机型

| 机型 | 代号 | 构建于 | 已实测 |
| --- | --- | --- | --- |
| 小米 15 Pro | `haotian` | OS4.0.0.19.XOBCNXM（Android 17） | 是 |

## 关于小米 15 Pro

以下都在真机上试过并可用：屏幕（包括子像素渲染——这块面板把它交给显示控制器）、
触摸屏与双击亮屏、adb 与 fastbootd、MTP、马达、WLAN，以及用锁屏密码解密 data 分区。

镜像基于的官方版本是 OS4.0.0.19.XOBCNXM。

### 如果仍然解不开

内核日志会说明卡在哪一环。先找这一行
`the blob the phone records is bound to a weaver; the blobs beside it are …`，然后：

- `the phone's weaver cannot answer` —— 传输链：`ssgtzd`、eSE 驱动，或安全元件 HAL。
- `the secure element does not recognise that credential` —— 密码不对。
- `the secure element is holding off further tries` —— 安全元件在多次错误尝试后限流。

如果问题在 DSP，日志里会有 `the ADSP is offline; asked the loader again now that
/firmware is in place`，随后要么 DSP 起来了，要么是
`it is not running, even after the loader was asked again`。

## 安装

需要先解锁 bootloader。从
[Releases](https://github.com/Rinrroy/skyward-for-haotian/releases) 下载镜像，
对照 `SHA256SUMS` 校验，然后在 fastboot 模式下把它刷到手机当前槽位的 recovery 分区：

```sh
sha256sum --check SHA256SUMS
fastboot flash recovery skyward-v0.2.1-haotian-OS4.0.0.19.XOBCNXM.img
fastboot reboot recovery
```

之后 Skyward 可以在「设置」的更新页里通过 Wi-Fi 从**本仓库**的最新版本在线更新；新镜像
也可以从它自己的「安装」页刷入。开启**再次刷入 Skyward** 后，写入 recovery 分区的系统
更新或刷机包会把 Skyward 重新装回去。

更新页向 `api.github.com/repos/Rinrroy/skyward-for-haotian` 询问最新版本，取名为
`skyward-<tag>-haotian-<stock>.img` 的资产——也就是镜像本身，直接以镜像发布，不再
套一层 zip。若你要再开一个分支，请改 `skyward_net::github::LATEST` 指向自己的仓库，
否则手机会被推给为 `haotian` 构建、但它用不了的镜像。

super 里没有系统时 Skyward 仍能启动，有屏幕、按键、adb、`adb sideload` 和 fastbootd，
足够装一个系统进去。触摸、WLAN 和解密都依赖已安装的系统，系统装好后即可用。

Skyward 里的 adb 是不需要授权的 root shell：拿到手机和线的人就能在 recovery 期间
访问它的存储。

## 双系统

Skyward 可以把第二个系统装在手机分区之外，并在首页的**双系统**里切换。第二个系统的
根是 data 分区里的一个文件，文件系统既不能移动也不能删除它；两套系统的 boot 镜像
也各存在这样的文件里。切换只写下一个启动槽位的 boot 与 init_boot 分区，每一步先落在
`/metadata` 上，所以断电打断的切换会在 Skyward 下次启动时补完。vendor_boot、dtbo、
recovery、vbmeta、固件和 super 都保持原样。

第二个系统从「安装」页的 zip 安装：

| 文件 | 内容 |
| --- | --- |
| `system.toml` | `id`、`name`、`version`、`filesystem`；`init`；`minimum`，根所需的最小空间 |
| `boot.img` | bootloader 启动的内核 |
| `rootfs.img` | 根文件系统的镜像 |
| `init_boot.img` | 仅用于自带 first ramdisk 的系统 |

没有自带 init_boot 的系统会用 Skyward 生成的：一个静态链接的程序，按已安装系统的
首阶段方式加载内核模块，在 data 分区里找到根、挂载并启动其中的 `init`。中途失败就
改为启动 Skyward，并说明原因。

## 构建

Skyward 用 `rust-toolchain.toml` 指定的稳定版 Rust、`aarch64-linux-android` 目标、
Android NDK 和 [cargo-ndk](https://github.com/bbqsrc/cargo-ndk) 构建。recovery 服务和
第二系统的首个程序针对手机构建，打包工具针对电脑构建：

```sh
cargo ndk -t arm64-v8a --platform 30 build --release -p skyward
RUSTFLAGS="-C target-feature=+crt-static" cargo ndk -t arm64-v8a --platform 30 build --release -p skyward-system-init --target-dir target/system-init
cargo build --release -p skyward-pack
```

打包工具从小米服务器上的完整更新包中取出官方 recovery 分区（只读取该分区所需的那部分），
并对照设备文件里的 SHA-256 校验：

```sh
target/release/skyward-pack stock --config devices/haotian.toml --out target/stock/haotian/recovery.img
target/release/skyward-pack stock --config devices/haotian.toml --out target/stock/haotian/recovery.img --partition odm
```

然后在它之上构建镜像：

```sh
target/release/skyward-pack recovery \
  --stock target/stock/haotian/recovery.img \
  --binary target/aarch64-linux-android/release/skyward \
  --system-init target/system-init/aarch64-linux-android/release/system-init \
  --config devices/haotian.toml \
  --font resources/fonts/MiSansVF.ttf \
  --mono resources/fonts/KirkMono-Regular.ttf \
  --mono-license resources/fonts/KirkMono-OFL.txt \
  --firmware cs40l26.wmfw=stock/firmware/cs40l26.wmfw \
  --firmware cs40l26.bin=stock/firmware/cs40l26.bin \
  --firmware cs40l26-svc.bin=stock/firmware/cs40l26-svc.bin \
  --firmware cs40l26-dvl.bin=stock/firmware/cs40l26-dvl.bin \
  --firmware cs40l26-dbc.bin=stock/firmware/cs40l26-dbc.bin \
  --firmware cs40l26-a2h.bin=stock/firmware/cs40l26-a2h.bin \
  --omapi resources/omapi \
  --out target/skyward-haotian.img
```

马达固件必须在 init 第一次探测之前就在 ramdisk 里，`--omapi` 则放入 Thales weaver
访问安全元件所需的 OMAPI 服务。`.github/workflows/build-haotian.yml` 会在打 tag 时
完成这些，并把镜像连同校验和一起发布。

测试和 lint 在电脑上跑；只在手机上运行的代码则针对手机构建后 lint：

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo ndk -t arm64-v8a --platform 30 clippy -p skyward -p skyward-install -p skyward-systems -p skyward-system-init -- -D warnings
```

Windows 主机可以交叉编译手机端、可以跑 `skyward-config` 的测试，但既不能打包镜像
（`skyward-bootimg` 用了 Unix 专属调用），也不能编译 `skyward-ui` 的测试
（`rsproperties` 在 Windows 上编译不过）。这两件事请用 Linux 或 CI。

`skyward-preview` 在电脑上把各页面画成 PNG，不需要手机。`--lang` 按 locale 选择语言，
例如 `en` 或 `zh-TW`：

```sh
cargo run --release -p skyward-preview -- target/preview --lang en
```

## 阅读本仓库

`AGENT.md` 是工作笔记：每部分知识花了多大代价才学到、诊断代码必须遵守的规矩，以及
针对这台手机的 weaver、ADSP 与面板为何如此。提交信息会详细解释**为什么**。

## 许可证

[GPL-3.0-or-later](LICENSE)
