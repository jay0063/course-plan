# 发布手册（Release Runbook）

本文是 CoursePlan 的**唯一发版操作真源**：从 bump 版本到校验产物、镜像与回滚，
全部按本文照做即可。相关文件：

| 文件 | 作用 |
| --- | --- |
| [`.github/workflows/ci.yml`](../.github/workflows/ci.yml) | 持续集成：push 到 `master`/`main` 与所有 PR 上跑 fmt / clippy / test |
| [`.github/workflows/release.yml`](../.github/workflows/release.yml) | 发版：tag `v*` 触发，构建 Windows 安装包 + 多架构服务端镜像 + 创建 GitHub Release（Linux / macOS 见下方开关） |
| [`.github/workflows/server.yml`](../.github/workflows/server.yml) | 服务端专项：crates 相关改动时跑服务端测试与 `docker compose` 冒烟 |
| [`packaging/`](../packaging) | 各平台本地打包脚本（CI 直接调用同一批脚本） |
| [`docs/DEPLOY.md`](DEPLOY.md) | 服务端镜像的部署 / 升级 / 备份 |

---

> ### ⚠️ 平台开关（2026-10-19 起）
>
> **Linux / macOS 客户端打包暂时停用**：`release.yml` 中 `linux` / `macos` 作业默认
> `if: false`（由 `workflow_dispatch` 的 `with_linux` / `with_macos` 控制，默认关闭）。
> 手动触发并勾选对应输入即可产出该平台产物；要恢复「每次发版都打包」，
> 删除这两个 job 的 `if:` 行并把它们加回 `release` 作业的 `needs:` 即可。
> 打包脚本（`packaging/linux`、`packaging/macos`）与本文其余内容保持有效。

## 1. 版本约定

* **唯一版本真源是根 `Cargo.toml` 的 `[package] version`**。
  所有平台脚本都从该字段读版本号拼产物名，`release.yml` 的 `checks` job 里有一道
  **版本守卫**：tag（去掉前导 `v`）必须与 `Cargo.toml` 的 version **完全相等**，
  否则整个发布流程在第一步就失败。
* tag 格式固定为 **`vX.Y.Z`**（例如 `v0.1.0`）。用 `0.x` 阶段语义化版本；
  `vX.Y.Z` 也用于筛选 release workflow 的触发（`tags: ["v*"]`）。
* 预发布版本（如 `v0.2.0-rc.1`）也会被解析出 `0.2.0-rc.1` 标签，
  但当前规则**同样会更新 `latest`**（见 §5）。如需要“预发布不移动 latest”，
  给 `release.yml` 里 `type=raw,value=latest` 加一条
  `enable=${{ !contains(env.RELEASE_TAG, '-') }}` 之类的条件即可。

---

## 2. 发版步骤（标准流程）

```bash
# 1) 确认工作区干净、CI 绿
git status
git switch main              # 或 master

# 2) bump 版本（唯一需要改的地方）
$EDITOR Cargo.toml           # [package] version = "0.2.0"
cargo check                  # ⚠️ 必须执行：Cargo.lock 里记录了根包版本，不同步更新
                             #    会让 CI 的 --locked 直接报 "lock file needs to be updated"

# 3) 本地过一遍发布前检查（与 CI 等价）
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked

# 4) 提交
git add Cargo.toml Cargo.lock
git commit -m "chore: release v0.2.0"
git push origin main

# 5) 打 tag 并推送（这一步才会触发 release workflow）
git tag v0.2.0
git push origin v0.2.0
```

```bash
# 6) 观察 workflow
gh run list  --workflow release --limit 5
gh run watch                    # 跟随最新一次运行

# 7) 校验产物与镜像
gh release view v0.2.0          # 资产清单应与 §4 一致
docker buildx imagetools inspect ghcr.io/<owner>/course-plan-server:v0.2.0
```

> ⚠️ **tag 与 Cargo.toml 不一致时**：直接删除远端 tag 后重打（见 §7 回滚），
> 不要用「再打一个 tag 覆盖」的方式绕过版本守卫——产物文件名会跟着 tag 走，
> 容易产生两个版本的资产混在同一个 release 里。
>
> ⚠️ `workflow_dispatch` 手动触发需要填 `tag` 输入（如 `v0.2.0`），
> 且该 tag 必须已存在、指向要发布的提交。常规发版**不要**用手动触发。

---

## 3. Secrets 配置

进入仓库 **Settings → Secrets and variables → Actions → New repository secret** 添加。

| Secret 名称 | 是否必需 | 用途 | 如何生成 |
| --- | --- | --- | --- |
| `APPLE_CERTIFICATE_P12` | 可选 | macOS 代码签名证书（base64 编码的 `.p12`） | 在「钥匙串访问」里导出 **Developer ID Application** 证书（含私钥）为 `.p12`，然后 `base64 -i cert.p12 \| pbcopy`，粘贴为 secret；也可以用 `base64 -i cert.p12 > cert.b64` 再复制文件内容 |
| `APPLE_CERTIFICATE_PASSWORD` | 可选 | 上述 `.p12` 的导出密码 | 导出时自己设定的密码 |
| `APPLE_SIGN_IDENTITY` | 可选 | 指定签名身份（如 `Developer ID Application: Foo Bar (ABCDE12345)`） | `security find-identity -v -p codesigning` 的输出里复制引号内文本。**不填时 CI 自动从导入的证书解析**（取第一个 `Developer ID Application:` 身份） |
| `APPLE_ID` | 可选 | 公证用的 Apple ID 邮箱 | 你的 Apple Developer 账号邮箱 |
| `APPLE_TEAM_ID` | 可选 | 公证用的 Team ID | <https://developer.apple.com/account> → Membership → Team ID（10 位） |
| `APPLE_APP_SPECIFIC_PASSWORD` | 可选 | 公证用的 App 专用密码 | <https://appleid.apple.com> → 登录 → 登录与安全 → App 专用密码 → 生成（形如 `abcd-efgh-ijkl-mnop`） |
| `DOCKERHUB_USERNAME` | 可选 | 额外把镜像同步推到 Docker Hub | Docker Hub 账号名；**不填则只推 GHCR** |
| `DOCKERHUB_TOKEN` | 可选 | Docker Hub 推送令牌 | Docker Hub → Account settings → Personal access tokens → 新建（Read & Write） |
| `WINDOWS_CERT_PFX_BASE64` / `WINDOWS_CERT_PASSWORD` | 未接入 | Windows 代码签名（见 §8 TODO） | — |

### 关键说明

* **推送到 GHCR 不需要任何额外 secret。** `server-image` job 用的是工作流自带的
  `secrets.GITHUB_TOKEN`（配合 job 级 `permissions: packages: write`），
  只要仓库允许 Actions 写包（默认允许），开箱即用。
  * 若组织策略禁用了 `GITHUB_TOKEN` 的包写权限，需在组织/仓库设置里放开，
    或改用 PAT——但默认配置下**不需要**。
* **Apple secrets 全部缺失 → macOS 走 ad-hoc 签名**，构建依然成功（`fail-fast: false`
  的两个架构都会产出 `.dmg` 与 `.app.tar.gz`）。用户首次打开需要
  **右键 → 打开**，或执行 `xattr -dr com.apple.quarantine /Applications/课程计划.app`。
* Apple secrets **部分缺失**时：有 `APPLE_CERTIFICATE_P12` 就签名；
  但 `APPLE_ID` / `APPLE_TEAM_ID` / `APPLE_APP_SPECIFIC_PASSWORD` 任一为空则**跳过公证**
  （`NOTARY_PROFILE` 为空，脚本不会调用 `notarytool`）。
  这种「已签名但未公证」的包在 macOS 上仍会被 Gatekeeper 拦，建议要么配齐、要么全不配。
* `if:` 里不能直接引用 `secrets.*`；`release.yml` 的做法是把 secret 先映射到 job 级
  `env:`，再用 `if: env.APPLE_CERTIFICATE_P12 != ''` / `if: env.DOCKERHUB_USERNAME != ''`
  判断。新增条件时请沿用这个模式。

---

## 4. 产物清单

`release.yml` 的 `release` job 会把所有 job 的 artifact 合并下载到 `release-assets/`
（`merge-multiple: true`），再用 `softprops/action-gh-release@v2` 一次性上传。

| 资产 | 来源 job | 平台 |
| --- | --- | --- |
| `course-plan_<version>_amd64.deb` | `linux` | Linux x86_64（Debian/Ubuntu） |
| `course-plan-<version>-x86_64.AppImage` | `linux` | Linux x86_64 免安装 |
| `course-plan-<version>-amd64.tar.gz` | `linux` | Linux x86_64 便携版 |
| `CoursePlan-Setup-<version>.exe` | `windows` | Windows 10/11 x64 安装包 |
| `CoursePlan-<version>-windows-x64.zip` | `windows` | Windows 10/11 x64 便携版 |
| `课程计划-<version>-macos-aarch64.dmg` | `macos` (aarch64 / `macos-14`) | macOS 11+ Apple Silicon |
| `课程计划-<version>-macos-x86_64.dmg` | `macos` (x86_64 / `macos-13`) | macOS 11+ Intel |
| `课程计划-<version>-macos-<arch>.app.tar.gz` | `macos` | 公证/校验用的 `.app` 归档 |

Release 说明正文由两部分组成：**自动生成的变更日志**（`generate_release_notes: true`）
与 workflow 里写死的自定义正文（资产说明、镜像拉取命令、未签名提示）——
GitHub 会把自定义正文放在前面、自动变更日志接在后面。
自定义正文里对 `docs/RELEASE.md`、`docs/DEPLOY.md` 的链接指向 **该 tag 对应的提交**，
所以文档改动请先合并、再打 tag。

### 发版前后检查清单

- [ ] `Cargo.toml` 版本 == tag（守卫会替你把关）
- [ ] `Cargo.lock` 已随版本号更新（`cargo check` 后一起提交，否则 `--locked` 会失败）
- [ ] `ci.yml` 在目标提交上是绿的
- [ ] tag 推送后 `checks` / `linux` / `windows` / `macos`(×2) / `server-image` 全绿
- [ ] Release 页面资产数量为 9（Linux 3 + Windows 2 + macOS 2 架构 × 2 文件）
- [ ] `docker pull ghcr.io/<owner>/course-plan-server:<tag>` 可拉取，`linux/amd64` + `linux/arm64` 都在
- [ ] 至少在一台真机上装一次对应平台的产物

---

## 5. 容器镜像（GHCR）

### 标签规则

由 `docker/metadata-action@v5` 从**发布 tag**推导（显式传 `value=`，不依赖 `github.ref`；
`flavor: latest=false` 表示 `latest` 完全由 workflow 显式控制）：

| 规则 | 对 tag `v0.2.1` 的结果 | 说明 |
| --- | --- | --- |
| `type=semver,pattern={{version}}` | `0.2.1` | 去掉 `v` 的语义化版本 |
| `type=semver,pattern={{major}}.{{minor}}` | `0.2` | 跟随后续补丁版本移动 |
| `type=raw,value=${{ env.RELEASE_TAG }}` | `v0.2.1` | tag 原文，方便与 Release 对照 |
| `type=raw,value=latest` | `latest` | 始终指向最新一次发布的 tag |

四条规则都显式传入发布 tag（`value=`）而不是依赖 `github.ref`，
这样在分支上手动 `workflow_dispatch` 也能得到同一组标签。

本轮新增的 4 个 tag 指向**同一个多架构 manifest list**，不是四份独立镜像。

### 多架构

`docker/setup-qemu-action@v3` + `docker/setup-buildx-action@v3` 之后由
`docker/build-push-action@v6` 以 `platforms: linux/amd64,linux/arm64` 一次构建推送
（构建上下文 `context: .`，Dockerfile `crates/server/Dockerfile`）。
ARM 层通过 QEMU 模拟构建，因此 `server-image` job 会比纯 amd64 构建慢一些。

`docker pull` 时 Docker 会自动挑当前架构对应的层；`docker inspect` 若显示
`Architecture: unknown/arm64,amd64` 说明拿到的是 manifest list，属正常现象。

### 本地验证镜像

```bash
# 只构建本机架构（快）
docker build -f crates/server/Dockerfile -t courseplan-server:dev .

# 查看远端标签与真实架构（不拉取全部层）
docker buildx imagetools inspect ghcr.io/<owner>/course-plan-server:0.2.1

# 起一台验证健康检查与 API
export JWT_SECRET="$(openssl rand -hex 32)"
docker run --rm -p 8080:8080 -e JWT_SECRET -v courseplan-data:/data courseplan-server:dev
curl -fsS http://127.0.0.1:8080/api/health      # {"status":"ok",...}
```

也可以直接复用仓库里的 compose（与 `server.yml` 的冒烟 job 完全一致）：

```bash
cp .env.example .env && $EDITOR .env            # 填 JWT_SECRET
docker compose up -d --build
curl -fsS http://127.0.0.1:8080/api/health
docker compose down -v
```

> Docker Hub 镜像（配置了 `DOCKERHUB_USERNAME` / `DOCKERHUB_TOKEN` 时）与 GHCR
> **标签完全相同**，把上面的 `ghcr.io/<owner>/course-plan-server` 换成
> `docker.io/<dockerhub-user>/course-plan-server` 即可。
> 未配置这两个 secret 时不会登录 Docker Hub，也不会推送，流程照常成功。

---

## 6. 本地等价命令 ↔ CI 对应关系

CI 的各平台 job 就是直接调用仓库里的打包脚本，因此**本地能把产物跑出来，
CI 就能跑出来**（差别只在系统依赖清单与环境变量）。

| CI job | CI 里的调用 | 本地等价命令 |
| --- | --- | --- |
| `checks` | `cargo fmt --all --check`；`cargo clippy --workspace --all-targets --locked -- -D warnings`；版本守卫读 `Cargo.toml` | 同样两条命令；版本守卫等价于要比 `git describe --tags` 与 `Cargo.toml` 的 version |
| `ci.yml` / `client` | 上述两条 + `cargo test --workspace --locked` | `cargo test --workspace --locked`；截图/无头渲染需 `COURSEPLAN_VISUAL=1 cargo test --test render`（无 GPU 会自动跳过） |
| `linux` | `cargo build --release --locked` → `./packaging/linux/build-deb.sh` → 下载 appimagetool 后 `./packaging/linux/build-appimage.sh` | `cargo build --release --locked && ./packaging/linux/build-deb.sh && ./packaging/linux/build-appimage.sh` |
| `windows` | `choco install innosetup -y` → `pwsh -File packaging/windows/build-windows.ps1` | 装好 Inno Setup 6 后 `pwsh -File packaging\windows\build-windows.ps1`（可加 `-StaticCrt` / `-SkipInstaller`） |
| `macos` | 导入证书 → `./packaging/macos/build-macos.sh`（`ARCH` 由 matrix 给出） | `ARCH=aarch64 ./packaging/macos/build-macos.sh` 或 `ARCH=x86_64 ./packaging/macos/build-macos.sh` |
| `server-image` | `docker buildx build --platform linux/amd64,linux/arm64 -f crates/server/Dockerfile .` | `docker buildx build --platform linux/amd64,linux/arm64 -t courseplan-server:dev --load .`（本机多架构查看用 `imagetools inspect`） |
| `server` / `test` | `cargo test -p course-plan-server --locked`；`cargo clippy -p course-plan-server --all-targets --locked -- -D warnings` | 同左 |
| `server` / `compose` | `docker compose up -d --build` + `curl /api/health` 轮询 60×2s | 同左，见 §5 |

macOS 本地做真实签名/公证：

```bash
security find-identity -v -p codesigning        # 找到 Developer ID Application 身份

# 一次性写入公证凭据（profile 名要与下面保持一致）
xcrun notarytool store-credentials "courseplan-notary" \
  --apple-id "$APPLE_ID" --team-id "$APPLE_TEAM_ID" \
  --password "$APPLE_APP_SPECIFIC_PASSWORD"

SIGN_IDENTITY="Developer ID Application: Foo Bar (ABCDE12345)" \
NOTARY_PROFILE="courseplan-notary" \
ARCH=aarch64 ./packaging/macos/build-macos.sh
```

> Linux 侧的 `libwayland-dev` / `libxkbcommon-x11-dev` / `libvulkan1` 等系统依赖
> 已写进 `release.yml` 与 `ci.yml` 的 “Install system dependencies” 步骤；
> 本地依赖清单见 README「快速开始 → 依赖」。

---

## 7. 回滚

发布产物挂在 tag 上，而 tag 一旦推送就不该被“改写历史”，所以回滚只有两条路：
**用新版本号重新发一次**（推荐），或删除 tag 后用同名 tag 覆盖产物（应急）。

### 7.1 产物有 bug，需要重新发布同一版本

```bash
# 1) 删掉本地与远端的错误 tag
git tag -d v0.2.0
git push origin :refs/tags/v0.2.0

# 2) 在修复提交上重新打同名 tag（版本守卫通过即代表 Cargo.toml 仍是 0.2.0）
git tag v0.2.0
git push origin v0.2.0
```

重新触发后 `softprops/action-gh-release@v2` 会**更新已存在的 Release**并覆盖同名资产。
想彻底重来就先删 Release（`gh release delete v0.2.0`），再按上面重打 tag。

> 重打同名 tag 会把 `latest`、`0.2`、`0.2.0`、`v0.2.0` 全部指向新的 manifest；
> 被顶掉的旧镜像变成悬空镜像，只有用 digest 才能再拉取。生产环境升级前建议先记录
> 旧 digest：`docker buildx imagetools inspect <image>:<tag>`。

### 7.2 已发布的版本有严重问题，需要撤回

```bash
gh release delete v0.2.0                 # 删除 Release（tag 可选保留：--cleanup-tag）
git push origin :refs/tags/v0.2.0        # 删除远端 tag，防止被再次引用

# 然后 bump 到一个新版本（如 0.2.1）走完整发版流程，而不是复用旧号
```

### 7.3 只回滚服务端容器

镜像不参与客户端版本回滚，直接使用上一个可用的 tag 即可：

```bash
docker pull ghcr.io/<owner>/course-plan-server:0.1.0
docker compose down && docker compose up -d        # 配合把 compose 里的 image 指向该 tag
```

数据卷 `courseplan-data` 不受影响；升级/降级的数据库兼容性见 `docs/DEPLOY.md`。

---

## 8. 已知缺口 / TODO

* **Windows 代码签名未接入**。`packaging/windows/build-windows.ps1` 尚未做
  `signtool` 签名，产物在目标机器上可能触发 SmartScreen「未知发布者」提示。
  接入时需要：
  1. 新增 secrets `WINDOWS_CERT_PFX_BASE64`（`.pfx` 的 base64）与 `WINDOWS_CERT_PASSWORD`；
  2. 在 `windows` job 里把 `.pfx` 还原到临时文件并 `Import-PfxCertificate`
     （或用 `signtool sign /f cert.pfx /p <pwd> /fd sha256 /tr <timestamp-url> /td sha256`）；
  3. 在 `build-windows.ps1` 里对 `course-plan.exe` **和** Inno Setup 安装包分别签名
     （先从内到外签 exe，再签 `.exe` 安装包）；
  4. 与 Apple 一样，用 job 级 `env:` 映射 secret，再用 `if: env.WINDOWS_CERT_PFX_BASE64 != ''`
     决定是否签名，保证没有证书时流程仍然成功。
* **Linux 无签名**（无 `.deb` 仓库 GPG 签名 / AppImage GPG 签名）。
* **AppImage 由 `appimagetool` 的 `continuous` 版本构建**——上游没有稳定 tag，
  构建可复现性弱于其他产物。
* **无自动 changelog 分类**：Release notes 用 GitHub 自动生成，
  建议坚持 conventional commits（`feat:` / `fix:` / `chore:`）以获得可读的分组。
