import { defineConfig } from 'vitepress'
import { withMermaid } from 'vitepress-plugin-mermaid'

// PostHog configuration
const POSTHOG_KEY = 'phc_BCRuie4xhbSduEL471w7XvQyPcP14QBXPidqdHYf4VY'
const POSTHOG_HOST = 'https://us.i.posthog.com'

// Support dual deployment:
// - GitHub Pages: /Ordo/docs/
// - Custom domain (docs.ordoengine.com): /
// Set DOCS_BASE_PATH env var to override, or use CUSTOM_DOMAIN=true for root path
const isCustomDomain = process.env.CUSTOM_DOMAIN === 'true'
const BASE_PATH = process.env.DOCS_BASE_PATH || (isCustomDomain ? '/' : '/Ordo/docs/')

const docs_en = [
  {
    text: 'Getting Started',
    items: [
        { text: 'What is Ordo?', link: '/en/guide/what-is-ordo' },
        { text: 'Quick Start', link: '/en/guide/quick-start' },
        { text: 'Install & Run', link: '/en/guide/getting-started' },
    ]
  },
  {
    text: 'Writing Rules',
    items: [
        { text: 'Rule Structure', link: '/en/guide/rule-structure' },
        { text: 'Expression Syntax', link: '/en/guide/expression-syntax' },
        { text: 'Built-in Functions', link: '/en/guide/builtin-functions' },
        { text: 'Decision Table', link: '/en/guide/decision-table' },
        { text: 'Execution Model', link: '/en/guide/execution-model' },
    ]
  },
  {
    text: 'Integration',
    items: [
        { text: 'HTTP API', link: '/en/api/http-api' },
        { text: 'gRPC API', link: '/en/api/grpc-api' },
        { text: 'WebAssembly', link: '/en/api/wasm' },
        { text: 'Data Filter API', link: '/en/api/filter-api' },
        { text: 'Capabilities & External Calls', link: '/en/guide/capabilities' },
    ]
  },
  {
    text: 'Operations',
    items: [
        { text: 'Rule Persistence', link: '/en/guide/persistence' },
        { text: 'Version Management', link: '/en/guide/versioning' },
        { text: 'Rule Signing', link: '/en/guide/rule-signing' },
        { text: 'Audit Logging', link: '/en/guide/audit-logging' },
        { text: 'Distributed Deployment', link: '/en/guide/distributed-deployment' },
        { text: 'Kubernetes', link: '/en/guide/integration/kubernetes' },
        { text: 'HashiCorp Nomad', link: '/en/guide/integration/nomad' },
    ]
  },
]

const reference_en = [
  {
    text: 'Reference',
    items: [
        { text: 'Server Options', link: '/en/reference/server-options' },
        { text: 'Configuration', link: '/en/reference/configuration' },
        { text: 'Metrics', link: '/en/reference/metrics' },
        { text: 'Benchmarks', link: '/en/reference/benchmarks' },
        { text: 'Roadmap', link: '/en/roadmap' },
    ]
  },
]

const docs_zh = [
  {
    text: '入门',
    items: [
        { text: 'Ordo 是什么？', link: '/zh/guide/what-is-ordo' },
        { text: '快速上手', link: '/zh/guide/quick-start' },
        { text: '安装与运行', link: '/zh/guide/getting-started' },
    ]
  },
  {
    text: '编写规则',
    items: [
        { text: '规则结构', link: '/zh/guide/rule-structure' },
        { text: '表达式语法', link: '/zh/guide/expression-syntax' },
        { text: '内置函数', link: '/zh/guide/builtin-functions' },
        { text: '决策表', link: '/zh/guide/decision-table' },
        { text: '执行模型', link: '/zh/guide/execution-model' },
    ]
  },
  {
    text: '接入',
    items: [
        { text: 'HTTP API', link: '/zh/api/http-api' },
        { text: 'gRPC API', link: '/zh/api/grpc-api' },
        { text: 'WebAssembly', link: '/zh/api/wasm' },
        { text: '数据过滤 API', link: '/zh/api/filter-api' },
        { text: '能力与外部调用', link: '/zh/guide/capabilities' },
    ]
  },
  {
    text: '运行与运维',
    items: [
        { text: '规则持久化', link: '/zh/guide/persistence' },
        { text: '版本管理', link: '/zh/guide/versioning' },
        { text: '规则签名', link: '/zh/guide/rule-signing' },
        { text: '审计日志', link: '/zh/guide/audit-logging' },
        { text: '分布式部署', link: '/zh/guide/distributed-deployment' },
        { text: 'Kubernetes', link: '/zh/guide/integration/kubernetes' },
        { text: 'HashiCorp Nomad', link: '/zh/guide/integration/nomad' },
    ]
  },
]

const reference_zh = [
  {
    text: '参考',
    items: [
        { text: '服务器选项', link: '/zh/reference/server-options' },
        { text: '配置', link: '/zh/reference/configuration' },
        { text: '指标', link: '/zh/reference/metrics' },
        { text: '性能基准', link: '/zh/reference/benchmarks' },
        { text: '路线图', link: '/zh/roadmap' },
    ]
  },
]

// https://vitepress.dev/reference/site-config
export default withMermaid(defineConfig({
  title: "Ordo",
  description: "An open-source rule engine that takes business rules out of your code",
  
  // Dynamic base path for dual deployment
  base: BASE_PATH,
  
  // Clean URLs without .html extension
  cleanUrls: true,

  // Internal repo-ops docs that ship in the tree but are not public pages
  srcExclude: ['DEPLOYMENT.md', '*/guide/editor-store.md'],
  
  // Last updated timestamp
  lastUpdated: true,
  
  head: [
    // Favicons - use relative paths that work with any base
    ['link', { rel: 'icon', type: 'image/x-icon', href: `${BASE_PATH}favicon.ico` }],
    ['link', { rel: 'icon', type: 'image/png', sizes: '32x32', href: `${BASE_PATH}favicon-32x32.png` }],
    ['link', { rel: 'icon', type: 'image/png', sizes: '16x16', href: `${BASE_PATH}favicon-16x16.png` }],
    ['link', { rel: 'apple-touch-icon', sizes: '180x180', href: `${BASE_PATH}apple-touch-icon.png` }],
    // PostHog Analytics
    ['script', {}, `
      !function(t,e){var o,n,p,r;e.__SV||(window.posthog=e,e._i=[],e.init=function(i,s,a){function g(t,e){var o=e.split(".");2==o.length&&(t=t[o[0]],e=o[1]),t[e]=function(){t.push([e].concat(Array.prototype.slice.call(arguments,0)))}}(p=t.createElement("script")).type="text/javascript",p.crossOrigin="anonymous",p.async=!0,p.src=s.api_host.replace(".i.posthog.com","-assets.i.posthog.com")+"/static/array.js",(r=t.getElementsByTagName("script")[0]).parentNode.insertBefore(p,r);var u=e;for(void 0!==a?u=e[a]=[]:a="posthog",u.people=u.people||[],u.toString=function(t){var e="posthog";return"posthog"!==a&&(e+="."+a),t||(e+=" (stub)"),e},u.people.toString=function(){return u.toString(1)+".people (stub)"},o="init capture register register_once register_for_session unregister unregister_for_session getFeatureFlag getFeatureFlagPayload isFeatureEnabled reloadFeatureFlags updateEarlyAccessFeatureEnrollment getEarlyAccessFeatures on onFeatureFlags onSessionId getSurveys getActiveMatchingSurveys renderSurvey canRenderSurvey getNextSurveyStep identify setPersonProperties group resetGroups setPersonPropertiesForFlags resetPersonPropertiesForFlags setGroupPropertiesForFlags resetGroupPropertiesForFlags reset get_distinct_id getGroups get_session_id get_session_replay_url alias set_config startSessionRecording stopSessionRecording sessionRecordingStarted captureException loadToolbar get_property getSessionProperty createPersonProfile opt_in_capturing opt_out_capturing has_opted_in_capturing has_opted_out_capturing clear_opt_in_out_capturing debug".split(" "),n=0;n<o.length;n++)g(u,o[n]);e._i.push([i,s,a])},e.__SV=1)}(document,window.posthog||[]);
      posthog.init('${POSTHOG_KEY}', {
        api_host: '${POSTHOG_HOST}',
        person_profiles: 'identified_only',
        capture_pageview: true,
        capture_pageleave: true
      });
    `],
  ],

  locales: {
    en: {
      label: 'English',
      lang: 'en',
      link: '/en/',
      themeConfig: {
        nav: [
          { text: 'Docs', link: '/en/guide/what-is-ordo', activeMatch: '^/en/(guide|api)/' },
          { text: 'Guard', link: '/en/platform/guard', activeMatch: '^/en/platform/guard' },
          { text: 'Studio & Platform', link: '/en/platform/overview', activeMatch: '^/en/platform/(?!guard)' },
          { text: 'Reference', link: '/en/reference/server-options', activeMatch: '^/en/(reference/|roadmap)' },
          { text: 'Playground', link: 'https://ordo-engine.github.io/Ordo/', target: '_self' },
        ],
        sidebar: {
          // Keys of equal depth resolve in insertion order, so the guard page
          // must be listed before the rest of /en/platform/.
          '/en/platform/guard': [
            {
              text: 'Ordo Guard',
              items: [
                  { text: 'Agent Guardrails', link: '/en/platform/guard' },
                  { text: 'CLI (ordo)', link: '/en/platform/cli' },
                  { text: 'MCP Server', link: '/en/platform/mcp' },
              ]
            },
          ],
          '/en/platform/': [
            {
              text: 'Platform',
              items: [
                  { text: 'Overview', link: '/en/platform/overview' },
                  { text: 'Quickstart', link: '/en/platform/quickstart' },
                  { text: 'Organizations & Projects', link: '/en/platform/organizations' },
                  { text: 'Studio Editor', link: '/en/platform/studio' },
              ]
            },
            {
              text: 'Develop & Integrate',
              items: [
                  { text: 'CLI (ordo)', link: '/en/platform/cli' },
                  { text: 'MCP Server', link: '/en/platform/mcp' },
                  { text: 'Runtime Integration', link: '/en/platform/integrate' },
                  { text: 'Traffic Capture & Replay', link: '/en/platform/capture-replay' },
              ]
            },
            {
              text: 'Modeling',
              items: [
                  { text: 'Fact Catalog', link: '/en/platform/catalog' },
                  { text: 'Decision Contracts', link: '/en/platform/contracts' },
                  { text: 'Sub-Rule Assets', link: '/en/platform/sub-rules' },
              ]
            },
            {
              text: 'Delivery',
              items: [
                  { text: 'Rule Drafts', link: '/en/platform/drafts' },
                  { text: 'Release Pipeline', link: '/en/platform/releases' },
                  { text: 'Test Management', link: '/en/platform/testing' },
              ]
            },
            {
              text: 'Operations',
              items: [
                  { text: 'Server Registry', link: '/en/platform/server-registry' },
                  { text: 'GitHub Integration', link: '/en/platform/github' },
              ]
            },
          ],
          '/en/guide/': docs_en,
          '/en/api/': docs_en,
          '/en/reference/': reference_en,
          '/en/roadmap': reference_en,
        },
        footer: {
          message: 'Released under the MIT License.',
          copyright: 'Copyright © 2024-present Ordo Contributors'
        },
        editLink: {
          pattern: 'https://github.com/Ordo-Engine/Ordo/edit/main/ordo-editor/apps/docs/:path',
          text: 'Edit this page on GitHub'
        },
        outline: {
          level: [2, 3],
          label: 'On this page'
        }
      }
    },
    zh: {
      label: '简体中文',
      lang: 'zh-Hans',
      link: '/zh/',
      title: "Ordo",
      description: "把业务规则从代码里拿出来的开源规则引擎",
      themeConfig: {
        nav: [
          { text: '文档', link: '/zh/guide/what-is-ordo', activeMatch: '^/zh/(guide|api)/' },
          { text: 'Guard', link: '/zh/platform/guard', activeMatch: '^/zh/platform/guard' },
          { text: 'Studio 与平台', link: '/zh/platform/overview', activeMatch: '^/zh/platform/(?!guard)' },
          { text: '参考', link: '/zh/reference/server-options', activeMatch: '^/zh/(reference/|roadmap)' },
          { text: '演练场', link: 'https://ordo-engine.github.io/Ordo/', target: '_self' },
        ],
        sidebar: {
          // Keys of equal depth resolve in insertion order, so the guard page
          // must be listed before the rest of /zh/platform/.
          '/zh/platform/guard': [
            {
              text: 'Ordo Guard',
              items: [
                  { text: 'Agent 护栏', link: '/zh/platform/guard' },
                  { text: '命令行 (ordo)', link: '/zh/platform/cli' },
                  { text: 'MCP 服务', link: '/zh/platform/mcp' },
              ]
            },
          ],
          '/zh/platform/': [
            {
              text: '平台',
              items: [
                  { text: '概览', link: '/zh/platform/overview' },
                  { text: '快速上手', link: '/zh/platform/quickstart' },
                  { text: '组织与项目', link: '/zh/platform/organizations' },
                  { text: 'Studio 编辑器', link: '/zh/platform/studio' },
              ]
            },
            {
              text: '开发与接入',
              items: [
                  { text: '命令行 (ordo)', link: '/zh/platform/cli' },
                  { text: 'MCP 服务', link: '/zh/platform/mcp' },
                  { text: '运行时接入', link: '/zh/platform/integrate' },
                  { text: '流量捕获与重放', link: '/zh/platform/capture-replay' },
              ]
            },
            {
              text: '建模',
              items: [
                  { text: '事实目录', link: '/zh/platform/catalog' },
                  { text: '决策契约', link: '/zh/platform/contracts' },
                  { text: '子规则资产', link: '/zh/platform/sub-rules' },
              ]
            },
            {
              text: '交付',
              items: [
                  { text: '规则草稿', link: '/zh/platform/drafts' },
                  { text: '发布流程', link: '/zh/platform/releases' },
                  { text: '测试管理', link: '/zh/platform/testing' },
              ]
            },
            {
              text: '运维',
              items: [
                  { text: '服务器注册', link: '/zh/platform/server-registry' },
                  { text: 'GitHub 集成', link: '/zh/platform/github' },
              ]
            },
          ],
          '/zh/guide/': docs_zh,
          '/zh/api/': docs_zh,
          '/zh/reference/': reference_zh,
          '/zh/roadmap': reference_zh,
        },
        footer: {
          message: '基于 MIT 许可发布。',
          copyright: '版权所有 © 2024-present Ordo 贡献者'
        },
        editLink: {
          pattern: 'https://github.com/Ordo-Engine/Ordo/edit/main/ordo-editor/apps/docs/:path',
          text: '在 GitHub 上编辑此页'
        },
        outline: {
          level: [2, 3],
          label: '本页目录'
        },
        lastUpdated: {
          text: '最后更新于'
        },
        docFooter: {
          prev: '上一页',
          next: '下一页'
        }
      }
    },
  },

  themeConfig: {
    // Logo - use small favicon icon
    logo: '/favicon-32x32.png',
    
    // Social links
    socialLinks: [
      { icon: 'github', link: 'https://github.com/Ordo-Engine/Ordo' }
    ],
    
    // Search
    search: {
      provider: 'local',
      options: {
        locales: {
          zh: {
            translations: {
              button: {
                buttonText: '搜索文档',
                buttonAriaLabel: '搜索文档'
              },
              modal: {
                noResultsText: '无法找到相关结果',
                resetButtonTitle: '清除查询条件',
                footer: {
                  selectText: '选择',
                  navigateText: '切换',
                  closeText: '关闭'
                }
              }
            }
          }
        }
      }
    }
  },

  mermaid: {},
}))
