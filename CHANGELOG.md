# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

* 支持压缩流

### Fixed

* 修复校验流状态的部分不起作用（始终成功）的问题

## [0.1.0] - 2023-12-26

### Added

* 自定义协议标识符和版本校验
* 支持不加密（安全的网络环境中）和加密（不安全的网络环境中）两种模式同时使用

### Deprecated

* `send_with_encrypt`和`recv_with_encrypt`方法始终使用相同的Nonce。
为防止重放攻击（Replay attack），请使用`client_send_with_dynamic_encrypt`、`server_recv_with_dynamic_encrypt`、`server_send_with_dynamic_encrypt`、`client_recv_with_dynamic_encrypt`加密解密。
这四个新方法将使用动态生成的Nonce。（旧方法将在下下个版本中被移除）
