[English](README.md) | [한국어](README.ko.md) | [日本語](README.ja.md) | **简体中文**

# bmo-eye

平台的视觉单元：三种模式共用一种请求与应答形式，每种模式都有自己的后端链。它本身不带任何模型——每种模式都
借助平台本地引擎已经在四个节点上运行的 Qwen3.8-Flash-Next 模型组来观看。

| Mode | CLI | What it reads |
| --- | --- | --- |
| watch | `bmo watch` | 视频：文件、视频或视频页面 URL（包括 YouTube）；场景、画面中的文字、语音 |
| see | `bmo see` | 图片：照片、截图、图表，可一次多张 |
| read | `bmo read` | 文档：扫描件、PDF、含文字和表格的页面（OCR） |

每种模式按顺序询问自己的链，采用第一个能正常工作的后端的回答：先是通用包节点上的本地引擎，然后是三个 Coder
节点，再是 OpenRouter，最后，在调用方提出要求且其模型能原生处理该输入时，才轮到调用方自己的模型。
视频的语音在工作台节点的 CPU 上识别，画面帧在本地引擎节点上读取；视觉单元不占用任何显卡。
除非另有说明，平台上的每个智能体查看图片或视频时都先经由 bmo-eye。

权威说明位于 `docs/`：API、各模式及其链、它所借助的引擎、计划，以及绘图请求的参考图流水线设计。归档文档和
呈给所有者的报告用韩语撰写；`docs/` 保持英文。

## 模型

仓库中不包含权重，它通过提供 OpenAI 兼容聊天 API 的引擎来"看"。

| 内容 | 来源 | 用途 |
|---|---|---|
| 视觉语言模型及其投影器 | Qwen3.8-Flash-Next 系列的 GGUF 与 `mmproj` 文件，由 GPU 节点上兼容 llama.cpp 的引擎提供（本部署使用的文件与节点见 `docs/engines.md`） | see、read 以及 watch 的画面 |
| 托管备用通道 | 任意 OpenAI 兼容服务商；本部署使用 OpenRouter 的 `qwen/qwen3.7-flash`，密钥放在已保存的密钥中 | 本地引擎无响应时的所有模式 |
| Whisper | `faster-whisper` `large-v3`（CTranslate2 int8），下载到节点的 Hugging Face 缓存 | 视频中的语音 |

各模型遵循各自的许可证，本仓库代码采用 MIT 许可。

## 结构

| Folder | Holds |
|---|---|
| `crates/` | Rust 工作区：`client`（类型与 HTTP 客户端）和 `server`（路由、存储、链、`bmo-eye` 二进制，以及写出 OpenAPI 文档的模块，由其 `openapi` 示例打印） |
| `docs/` | 文档索引、API 及由其生成的 OpenAPI 文档、各模式及其链、引擎、计划、绘图参考设计，以及三个工具条目 `eye/see`、`eye/read` 和 `eye/watch` |
| `scripts/` | 第一阶段数据测量时使用的测量工具 |
| `src/` | 仓库自身的包，不含代码，只包含横跨各 crate 的测试 |
| `tests/` | 针对替身引擎节点走查路由的测试、读取构成一次查看（look）的媒体的测试，以及让保存的 OpenAPI 文档与代码输出保持一致的测试 |
| `deploy/` | 部署时重启并监视的用户单元 |

根目录下有 Cargo 工作区、工具宿主的清单 `tool.toml` 及其条目引用的 schema（按宿主要求放在其旁边）、testbed 的
`testbed.toml`、`install.toml`（在新机器上安装本单元所需的内容），以及 `CLAUDE.md`。
