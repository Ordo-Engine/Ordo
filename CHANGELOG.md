# Changelog

All notable changes to Ordo will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Documentation

- Add benchmark race GIF to README Performance section ([#42](https://github.com/Pama-Lee/Ordo/pull/42))
([6b0c57b](https://github.com/Pama-Lee/Ordo/commit/6b0c57b23dbe9308ce5e6178dcc4d31cfbc17ee3))
- Add benchmark visualization and competitive comparison data ([#40](https://github.com/Pama-Lee/Ordo/pull/40))
([f5dba2e](https://github.com/Pama-Lee/Ordo/commit/f5dba2e026bfde43c157061ea156e6c38d5bb27c))

### Features

- **ordo-core:** Add 23 extended built-in functions ([#52](https://github.com/Pama-Lee/Ordo/pull/52))
([7860cfa](https://github.com/Pama-Lee/Ordo/commit/7860cfaefbfbcd30ee5f9086d6b98962b0ad83e2))
- **ordo-core:** Add 30+ extended built-in functions ([#48](https://github.com/Pama-Lee/Ordo/pull/48))
([b4adbbc](https://github.com/Pama-Lee/Ordo/commit/b4adbbc76375e480a10b7e3f8302f41f501ae6c7))
- **ordo-server:** Add external reference data store ([#50](https://github.com/Pama-Lee/Ordo/pull/50))
([d3f071f](https://github.com/Pama-Lee/Ordo/commit/d3f071f5731aea3d3ed1274da202c5a840fc5e6e))
- **rule-composition:** Add CallRuleSet action and pipeline API ([#51](https://github.com/Pama-Lee/Ordo/pull/51))
([47354c2](https://github.com/Pama-Lee/Ordo/commit/47354c21e5bfc434e7ba0542a0390531228c91b7))
- **sdk:** Add Python SDK with HTTP/gRPC support ([#53](https://github.com/Pama-Lee/Ordo/pull/53))
([3aafb56](https://github.com/Pama-Lee/Ordo/commit/3aafb56297a459153c4b2f9489cf4bd5e9f8a7f6))
- **server:** Add store resource limits (max_rules_per_tenant, max_total_rules) ([#38](https://github.com/Pama-Lee/Ordo/pull/38))
([bc7be7f](https://github.com/Pama-Lee/Ordo/commit/bc7be7fe7fd270b8858b25205eeb4e0dfd0d2750))
- **server:** Add config validation, sync metrics, and instance ID improvement ([#36](https://github.com/Pama-Lee/Ordo/pull/36))
([4725269](https://github.com/Pama-Lee/Ordo/commit/4725269e395c590a87b22898f4ed08d7e6b9c099))
- **server:** Add NATS JetStream sync for distributed deployments ([#34](https://github.com/Pama-Lee/Ordo/pull/34))
([d08d400](https://github.com/Pama-Lee/Ordo/commit/d08d40098d2c51806e1116fb1b3770bfa595bdc1))

### Performance

- **server:** Optimize HTTP serialization and reduce lock contention ([#43](https://github.com/Pama-Lee/Ordo/pull/43))
([1e4415a](https://github.com/Pama-Lee/Ordo/commit/1e4415a29874d96c90809597af5e5f2c6e733b79))

## [0.3.0] - 2026-03-06

### Bug Fixes

- **ci:** Skip JIT benchmarks in GitHub Actions
([54c9c06](https://github.com/Pama-Lee/Ordo/commit/54c9c0654d5eeef75146c3ffff19704a4b45c95f))
- **docs:** Use dynamic base path for language redirect
([ad64f9c](https://github.com/Pama-Lee/Ordo/commit/ad64f9c2f7d55cde9b473b97c97d59153473ac1a))
- Improve error handling and add project skills
([18e9832](https://github.com/Pama-Lee/Ordo/commit/18e98320e341bcbedd0e84a85571ac912dd00249))
- Make signature feature optional for WASM compatibility
([ed676a5](https://github.com/Pama-Lee/Ordo/commit/ed676a560dd693b3851832182de1ecd4ca05793d))

### Documentation

- Add integration guides for Nomad and Kubernetes
([10ecea7](https://github.com/Pama-Lee/Ordo/commit/10ecea7b520b623480b7cfa73d2c43aad01cd622))
- Update documentation for v0.2.0 release
([39d1dfa](https://github.com/Pama-Lee/Ordo/commit/39d1dfa4bf2393684066b9adeee2435ca382cb7c))

### Features

- **grpc:** Add multi-tenancy support and batch execution ([#26](https://github.com/Pama-Lee/Ordo/pull/26))
([445888d](https://github.com/Pama-Lee/Ordo/commit/445888df85e4da95798339043fc13dba85acebd1))
- **playground:** Add .ordo file import/export support
([dcf2383](https://github.com/Pama-Lee/Ordo/commit/dcf238316555abe883ac4b595dde56ac5344f848))
- **server:** Add Writer/Reader role deployment, file watcher, K8s health probes, and request limits ([#30](https://github.com/Pama-Lee/Ordo/pull/30))
([1a6deef](https://github.com/Pama-Lee/Ordo/commit/1a6deef44b7aef6c9c1b4be223b28f36824afd23))
- **server:** Graceful shutdown, panic recovery, and OpenTelemetry ([#27](https://github.com/Pama-Lee/Ordo/pull/27))
([257878c](https://github.com/Pama-Lee/Ordo/commit/257878c326758b2ea4af560677878e52dd735e58))
- **server:** Add multi-tenancy support with namespace isolation
([5b4cef7](https://github.com/Pama-Lee/Ordo/commit/5b4cef79197e855d6b33e08d0a31502256addc31))
- Add expression input limits and HTTP API integration tests ([#28](https://github.com/Pama-Lee/Ordo/pull/28))
([c5663be](https://github.com/Pama-Lee/Ordo/commit/c5663becf948b1411b76fb40cd1db2cf894ff19d))
- Add Ed25519 rule signature verification
([83a13f7](https://github.com/Pama-Lee/Ordo/commit/83a13f788b3668a2d3598a5e6bc903d074d5c079))

## [0.2.0] - 2026-01-18

### Bug Fixes

- **docs:** Add CUSTOM_DOMAIN env var to vercel.json
([ecf6e37](https://github.com/Pama-Lee/Ordo/commit/ecf6e37d3d3ce7127936456bf593eac40c2ac491))
- **docs:** Add vercel.json with correct output directory
([f9e9657](https://github.com/Pama-Lee/Ordo/commit/f9e96575c84575902d46389eb8fc35bbb500d7da))
- **playground:** Use dynamic VERSION from editor-core
([5b80f34](https://github.com/Pama-Lee/Ordo/commit/5b80f344e49df3f784e906af903f61dc7416c265))
- **wasm:** Make JIT feature optional for wasm32 target
([8780814](https://github.com/Pama-Lee/Ordo/commit/87808145e075753f24943242c96f9f025fe29751))

### Documentation

- Update README and add dual-domain deployment support
([e204527](https://github.com/Pama-Lee/Ordo/commit/e20452764aefe8598a38aab95c736d35318bfa87))
- Fix dead links in quick-start.md
([a83fecf](https://github.com/Pama-Lee/Ordo/commit/a83fecfae8d4b1f2f272e984893d8c27bd9f64c7))

### Features

- **expr:** Implement expression optimization techniques
([04e2cf5](https://github.com/Pama-Lee/Ordo/commit/04e2cf5bc2e84868b7568e626af4130fc868e116))
- **jit:** Implement schema-based JIT compilation system
([d2d97f8](https://github.com/Pama-Lee/Ordo/commit/d2d97f8f07e690a9048bab1b8a7875211a606f21))
- **npm:** Setup npm publishing with changesets
([18f1f1b](https://github.com/Pama-Lee/Ordo/commit/18f1f1bd2b2ecb7edf33c7b859384b713db855ac))
- Implement silent JIT compilation system
([570211a](https://github.com/Pama-Lee/Ordo/commit/570211a36238cfefd68a0fceff610da85af95efc))
- Add VM visualization debug system with ruleset debugging support
([372aad9](https://github.com/Pama-Lee/Ordo/commit/372aad91484d45b4e02b00b7f71dbb1df62d81e1))
- Add batch execution API for improved throughput
([e54c6ef](https://github.com/Pama-Lee/Ordo/commit/e54c6ef966f9724d02c8d3efea9a8cdcedc54652))
- Add lightweight Prometheus metrics for better observability
([da02a54](https://github.com/Pama-Lee/Ordo/commit/da02a54187a3da3b95d3ba0df52c999097f4911a))

### Miscellaneous

- Bump version to 0.2.0
([58b4086](https://github.com/Pama-Lee/Ordo/commit/58b40866e11bbf8c87dd1db8134a430b369433da))
- Remove benchmark results from git tracking
([1d959ff](https://github.com/Pama-Lee/Ordo/commit/1d959ffce1e82bd073738c0e36fe27f043c7fc65))

### Refactor

- **release:** Simplify npm publishing workflow
([8dbf510](https://github.com/Pama-Lee/Ordo/commit/8dbf5102e1db87c31408e736db4966f6a84bfe56))

## [0.1.8] - 2026-01-14

### Features

- **docs:** Enhance documentation with multilingual support and new guides
([17e9ab8](https://github.com/Pama-Lee/Ordo/commit/17e9ab81553b703874392a4d048b432d0199027b))

### Performance

- CPU efficiency optimizations for rule engine
([6b39377](https://github.com/Pama-Lee/Ordo/commit/6b393776d1c28cd74c4f4489538dde1cf44a8a4a))

## [0.1.7] - 2026-01-12

### Features

- **core:** Implement MetricSink trait for custom rule metrics integration
([f014700](https://github.com/Pama-Lee/Ordo/commit/f014700d30b11005087692c2318438cdd12a64c7))
- Add favicons to Playground and Docs, add PostHog analytics to VitePress
([3e21c5a](https://github.com/Pama-Lee/Ordo/commit/3e21c5abff075df2d416fba8d865206f9b9b9690))

## [0.1.6] - 2026-01-12

### Bug Fixes

- **docs:** Include VitePress config.mts in git (was ignored)
([5d53312](https://github.com/Pama-Lee/Ordo/commit/5d53312b62ec53ba1840adb4538cb181de4cfa18))
- **docs:** Wrap localhost URL in code backticks to avoid dead link error
([cda6b8e](https://github.com/Pama-Lee/Ordo/commit/cda6b8e94a8d12aefd68bb295208b1d599249910))
- **docs:** Correct formatting of PromQL examples in metrics documentation
([f4ee5ea](https://github.com/Pama-Lee/Ordo/commit/f4ee5ea51ad6f47fa891568b733dde40e0d7ee85))
- Resolve clippy warnings and enhance pre-commit hook with clippy check
([a7ccf2e](https://github.com/Pama-Lee/Ordo/commit/a7ccf2e3f50589e08f9e4853a66d5cf36664ec17))
- Add WASM stub for playground build without Rust
([49b1171](https://github.com/Pama-Lee/Ordo/commit/49b1171a92f9a4f40a7d340ed941b94262bd3407))

### Documentation

- Update README with visual editor screenshots
([629feb2](https://github.com/Pama-Lee/Ordo/commit/629feb2f9f389dcdf43308a21fe269d46c7a785d))

### Features

- **docs:** Add comprehensive documentation for Ordo rule engine
([8c6bedd](https://github.com/Pama-Lee/Ordo/commit/8c6bedd8ec78e2eba72ea9b39beb481c0fda92ae))
- **playground:** Add PostHog analytics integration
([8e79612](https://github.com/Pama-Lee/Ordo/commit/8e796129d509efa8440ee559e2f70dd8884df4cd))
- **server:** Add structured audit logging with dynamic sample rate
([ef651d5](https://github.com/Pama-Lee/Ordo/commit/ef651d5a0d77cac6365396ba13be8d7e39f17d05))
- **server:** Add rule versioning with rollback support
([ec29570](https://github.com/Pama-Lee/Ordo/commit/ec29570aed40e613396389c1dcd3ee7883dbbab9))
- Add Prometheus metrics and enhanced health check endpoint
([d92db86](https://github.com/Pama-Lee/Ordo/commit/d92db86e1d945834568a5f6b8a5378b74a4aa1c1))
- Implement file-based rule persistence in Ordo server
([c64d0f5](https://github.com/Pama-Lee/Ordo/commit/c64d0f55fec57687d94843ab372c82f679f58461))
- Build WASM in CI for GitHub Pages deployment
([d971573](https://github.com/Pama-Lee/Ordo/commit/d971573e3337bbbe80865c58bc4b2824576d8fba))
- Add GitHub Pages deployment for playground
([65c2043](https://github.com/Pama-Lee/Ordo/commit/65c2043f1bb2072cb60980cea62aab7cd1de8f34))
- Add GitHub Pages deployment for playground
([5aa77d4](https://github.com/Pama-Lee/Ordo/commit/5aa77d480b37a74cd19f3be31fdc69a5e9151905))

### Miscellaneous

- Add driver.js dependency to pnpm-lock.yaml
([7054f98](https://github.com/Pama-Lee/Ordo/commit/7054f98a709553745d36eef18d777ccf0af017ae))
- Update dependency installation command in deploy-playground workflow
([cc70797](https://github.com/Pama-Lee/Ordo/commit/cc707978f465cc0114ffd766d1a8429f0e59da1d))
- Update Dockerfile to include protoc and curl installation
([61f77c1](https://github.com/Pama-Lee/Ordo/commit/61f77c137b88a9af1a5ad77d1410c654776f62f2))
- Remove obsolete and engine integration summaries
([848f899](https://github.com/Pama-Lee/Ordo/commit/848f899cd5326662c7c3ad00daa0e47b910dae53))

### Style

- Apply cargo fmt and add pre-commit hook for auto-formatting
([9b0b306](https://github.com/Pama-Lee/Ordo/commit/9b0b30658d61967fb4cbe09c469b2f94e38fc797))

## [0.1.0] - 2026-01-07

### Bug Fixes

- Use std::slice::from_ref instead of clone in slice
([b3ef2a9](https://github.com/Pama-Lee/Ordo/commit/b3ef2a959c7dfd8eead141e62b11ddbcc9298f9b))
- Resolve all clippy warnings and formatting issues
([cfa2cd5](https://github.com/Pama-Lee/Ordo/commit/cfa2cd595281d06ab8f266ddc5d157a1b9e5192c))
- Correct rust-toolchain action name in CI workflows
([09912cd](https://github.com/Pama-Lee/Ordo/commit/09912cd38c6e47242da0c4b6057f611377fcc7a3))
- Correct rust-toolchain action name in CI workflows
([d4d269e](https://github.com/Pama-Lee/Ordo/commit/d4d269e9ce42add976bae0a096918e7f7dc064e6))
- Correct git clone URL in README
([97522b6](https://github.com/Pama-Lee/Ordo/commit/97522b6c12b2183623b843c18c190dae022a2c64))

### Documentation

- Add branch strategy to README
([fe35eb5](https://github.com/Pama-Lee/Ordo/commit/fe35eb5bb120321d9af3aaa53207acd98b58a5fb))

### Features

- Add GitHub Actions CI/CD and Docker support
([7c60909](https://github.com/Pama-Lee/Ordo/commit/7c60909866585140d0b20c7770515587dc991bf1))

### Style

- Apply rustfmt formatting to all source files
([e0232ff](https://github.com/Pama-Lee/Ordo/commit/e0232ff212068c9c4cbb4ffd8e49878bb523e239))

[Unreleased]: https://github.com/Pama-Lee/Ordo/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/Pama-Lee/Ordo/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/Pama-Lee/Ordo/compare/v0.1.8...v0.2.0
[0.1.8]: https://github.com/Pama-Lee/Ordo/compare/v0.1.7...v0.1.8
[0.1.7]: https://github.com/Pama-Lee/Ordo/compare/v0.1.6...v0.1.7
[0.1.6]: https://github.com/Pama-Lee/Ordo/compare/v0.1.0...v0.1.6

