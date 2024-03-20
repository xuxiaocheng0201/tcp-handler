# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.6.0] - 2024-3-20

### Added

* 增加`compress_encryption`feature。
* 增加对于recv的处理模型。

### Changed

* 重构协议，尽可能实现0拷贝。
* 耗时同步代码分离，使用[tokio::task::block_in_place](https://docs.rs/tokio/1.36.0/tokio/task/fn.block_in_place.html)。
* 简化cipher使用。
* 移除Send标记。
* 优化文档。
* 更新依赖。

## [0.5.3] - 2024-1-27

### Added

* config支持序列化/反序列化。

### Changed

* 更新依赖。

## [0.5.2] - 2024-1-26

### Changed

* 再次更新依赖。

## [0.5.1] - 2024-1-25

### Changed

* 更新依赖。

## [0.5.0] - 2023-12-28

### Added

* 支持发送不连续的buf（chain）。

### Fixed

* 完善Nonce部分的协议解析。

## [0.4.0] - 2023-12-27

### Added

* 支持便捷地从`StarterError`中提取`std::io::Error`。

### Changed

* 添加`client_init`速度极慢的提示和解决办法。
* 重新组织代码结构。
* 完善文档，添加协议解析。

## [0.3.2] - 2023-12-27

### Changed

* 开放更多extern。

### Fixed

* 开放`AesCipher`类型。

## [0.3.1] - 2023-12-26

### Changed

* 自定义版本校验时使用`FnOnce`代替`Fn`，以此从中提取版本。

## [0.3.0] - 2023-12-26

### Changed

* 重新组织开放方法结构，减少方法名长度。

### Removed

* 移除上两个版本中多余的加密方法。

## [0.2.0] - 2023-12-26

### Added

* 支持压缩流。
* 支持压缩加密双态流。

### Fixed

* 修复校验流状态的部分不起作用（始终成功）的问题。

### Deprecated

* 弃用上个版本添加的四个各不相同的方法，减少冗余。
请使用`send_with_dynamic_encrypt`和`recv_with_dynamic_encrypt`这两个相同的方法收发消息。
旧方法将在`0.3.0`中被移除。

### Security

* 修复Nonce明文传输的漏洞。
* 修复Nonce在客户端侧重复使用的漏洞。

## [0.1.0] - 2023-12-26

### Added

* 自定义协议标识符和版本校验。
* 支持不加密（安全的网络环境中）和加密（不安全的网络环境中）两种模式同时使用。

### Deprecated

* `send_with_encrypt`和`recv_with_encrypt`方法始终使用相同的Nonce。
为防止重放攻击（Replay attack），请使用`client_send_with_dynamic_encrypt`、`server_recv_with_dynamic_encrypt`、`server_send_with_dynamic_encrypt`、`client_recv_with_dynamic_encrypt`加密解密。
这四个新方法将使用动态生成的Nonce。旧方法将在`0.3.0`中被移除。
