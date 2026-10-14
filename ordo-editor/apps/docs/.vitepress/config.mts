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

const sidebar_en = [
  {
    text: 'Getting Started',
    items: [
      { text: 'What is Ordo?', link: '/en/guide/what-is-ordo' },
      { text: 'Quick Start', link: '/en/guide/quick-start' },
    ]
  },
  {
    text: 'Writing Rules',
    items: [
      { text: 'Rule Structure', link: '/en/guide/rule-structure' },
      { text: 'Expressions', link: '/en/guide/expression-syntax' },
      { text: 'Decision Tables', link: '/en/guide/decision-table' },
      { text: 'Built-in Functions', link: '/en/guide/builtin-functions' },
    ]
  },
  {
    text: 'Testing and Impact',
    items: [
      { text: 'Test, Trace, Impact', link: '/en/guide/testing' },
    ]
  },
  {
    text: 'With a Decision Model',
    items: [
      { text: 'Decision Model Gateway', link: '/en/platform/decision-gateway' },
    ]
  },
  {
    text: 'Running It',
    items: [
      { text: 'Running the Server', link: '/en/guide/server' },
      { text: 'Deployment', link: '/en/guide/deployment' },
      { text: 'Embedding', link: '/en/guide/embed' },
    ]
  },
  {
    text: 'Studio and Platform',
    items: [
      { text: 'Overview', link: '/en/platform/overview' },
      { text: 'Studio', link: '/en/platform/studio' },
      { text: 'Facts, Concepts, Contracts', link: '/en/platform/modeling' },
      { text: 'Drafts and Releases', link: '/en/platform/releases' },
      { text: 'Engines', link: '/en/platform/servers' },
    ]
  },
  {
    text: 'Coding Agents',
    items: [
      { text: 'Guard', link: '/en/platform/guard' },
      { text: 'MCP Server', link: '/en/platform/mcp' },
    ]
  },
  {
    text: 'Reference',
    items: [
      { text: 'HTTP API', link: '/en/api/http-api' },
      { text: 'gRPC API', link: '/en/api/grpc-api' },
      { text: 'Server Options', link: '/en/reference/server-options' },
      { text: 'CLI', link: '/en/platform/cli' },
      { text: 'Metrics', link: '/en/reference/metrics' },
      { text: 'Benchmarks', link: '/en/reference/benchmarks' },
    ]
  },
]

const sidebar_zh = [
  {
    text: '入门',
    items: [
      { text: 'Ordo 是什么？', link: '/zh/guide/what-is-ordo' },
      { text: '快速上手', link: '/zh/guide/quick-start' },
    ]
  },
  {
    text: '编写规则',
    items: [
      { text: '规则结构', link: '/zh/guide/rule-structure' },
      { text: '表达式', link: '/zh/guide/expression-syntax' },
      { text: '决策表', link: '/zh/guide/decision-table' },
      { text: '内置函数', link: '/zh/guide/builtin-functions' },
    ]
  },
  {
    text: '测试与影响',
    items: [
      { text: '测试、追踪、影响分析', link: '/zh/guide/testing' },
    ]
  },
  {
    text: '配合决策模型',
    items: [
      { text: '决策模型网关', link: '/zh/platform/decision-gateway' },
    ]
  },
  {
    text: '运行',
    items: [
      { text: '运行服务', link: '/zh/guide/server' },
      { text: '部署', link: '/zh/guide/deployment' },
      { text: '嵌入使用', link: '/zh/guide/embed' },
    ]
  },
  {
    text: 'Studio 与平台',
    items: [
      { text: '概览', link: '/zh/platform/overview' },
      { text: 'Studio', link: '/zh/platform/studio' },
      { text: '事实、概念、契约', link: '/zh/platform/modeling' },
      { text: '草稿与发布', link: '/zh/platform/releases' },
      { text: '引擎', link: '/zh/platform/servers' },
    ]
  },
  {
    text: '编码 Agent',
    items: [
      { text: 'Guard', link: '/zh/platform/guard' },
      { text: 'MCP 服务', link: '/zh/platform/mcp' },
    ]
  },
  {
    text: '参考',
    items: [
      { text: 'HTTP API', link: '/zh/api/http-api' },
      { text: 'gRPC API', link: '/zh/api/grpc-api' },
      { text: '服务器选项', link: '/zh/reference/server-options' },
      { text: '命令行', link: '/zh/platform/cli' },
      { text: '指标', link: '/zh/reference/metrics' },
      { text: '性能基准', link: '/zh/reference/benchmarks' },
    ]
  },
]

// https://vitepress.dev/reference/site-config
export default withMermaid(defineConfig({
  title: "Ordo",
  description: "Ordo is an open-source decision engine. It runs tested, versioned rules on your data and on a decision model's answers.",
  
  // Dynamic base path for dual deployment
  base: BASE_PATH,
  
  // Clean URLs without .html extension
  cleanUrls: true,

  // Repo-ops notes that ship in the tree but are not a public page
  srcExclude: ['DEPLOYMENT.md'],
  
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
          { text: 'Docs', link: '/en/guide/what-is-ordo', activeMatch: '^/en/(guide/|platform/decision-gateway)' },
          { text: 'Studio & Platform', link: '/en/platform/overview', activeMatch: '^/en/platform/(overview|studio|modeling|releases|servers)' },
          { text: 'Guard', link: '/en/platform/guard', activeMatch: '^/en/platform/(guard|mcp)' },
          { text: 'Reference', link: '/en/api/http-api', activeMatch: '^/en/(api/|reference/|platform/cli)' },
          { text: 'Playground', link: 'https://ordo-engine.github.io/Ordo/', target: '_self' },
        ],
        sidebar: sidebar_en,
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
      description: "Ordo 是开源的决策引擎，用带测试、带版本的规则处理业务数据和决策模型的回答。",
      themeConfig: {
        nav: [
          { text: '文档', link: '/zh/guide/what-is-ordo', activeMatch: '^/zh/(guide/|platform/decision-gateway)' },
          { text: 'Studio 与平台', link: '/zh/platform/overview', activeMatch: '^/zh/platform/(overview|studio|modeling|releases|servers)' },
          { text: 'Guard', link: '/zh/platform/guard', activeMatch: '^/zh/platform/(guard|mcp)' },
          { text: '参考', link: '/zh/api/http-api', activeMatch: '^/zh/(api/|reference/|platform/cli)' },
          { text: '演练场', link: 'https://ordo-engine.github.io/Ordo/', target: '_self' },
        ],
        sidebar: sidebar_zh,
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
