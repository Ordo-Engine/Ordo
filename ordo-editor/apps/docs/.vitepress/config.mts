import { defineConfig } from 'vitepress'

// PostHog configuration
const POSTHOG_KEY = 'phc_BCRuie4xhbSduEL471w7XvQyPcP14QBXPidqdHYf4VY'
const POSTHOG_HOST = 'https://us.i.posthog.com'

// https://vitepress.dev/reference/site-config
export default defineConfig({
  title: "Ordo",
  description: "High-performance rule engine with visual editor",
  
  // Deploy to /Ordo/docs/ on GitHub Pages
  base: '/Ordo/docs/',
  
  // Clean URLs without .html extension
  cleanUrls: true,
  
  // Last updated timestamp
  lastUpdated: true,
  
  head: [
    // Favicons
    ['link', { rel: 'icon', type: 'image/x-icon', href: '/Ordo/docs/favicon.ico' }],
    ['link', { rel: 'icon', type: 'image/png', sizes: '32x32', href: '/Ordo/docs/favicon-32x32.png' }],
    ['link', { rel: 'icon', type: 'image/png', sizes: '16x16', href: '/Ordo/docs/favicon-16x16.png' }],
    ['link', { rel: 'apple-touch-icon', sizes: '180x180', href: '/Ordo/docs/apple-touch-icon.png' }],
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
          { text: 'Guide', link: '/en/guide/getting-started' },
          { text: 'API', link: '/en/api/http-api' },
          { text: 'Reference', link: '/en/reference/cli' },
          { 
            text: 'Playground', 
            link: 'https://pama-lee.github.io/Ordo/',
            target: '_self'
          },
        ],
        sidebar: {
          '/en/guide/': [
            {
              text: 'Introduction',
              items: [
                { text: 'What is Ordo?', link: '/en/guide/what-is-ordo' },
                { text: 'Getting Started', link: '/en/guide/getting-started' },
                { text: 'Quick Start', link: '/en/guide/quick-start' },
              ]
            },
            {
              text: 'Core Concepts',
              items: [
                { text: 'Rule Structure', link: '/en/guide/rule-structure' },
                { text: 'Expression Syntax', link: '/en/guide/expression-syntax' },
                { text: 'Built-in Functions', link: '/en/guide/builtin-functions' },
              ]
            },
            {
              text: 'Features',
              items: [
                { text: 'Rule Persistence', link: '/en/guide/persistence' },
                { text: 'Version Management', link: '/en/guide/versioning' },
                { text: 'Audit Logging', link: '/en/guide/audit-logging' },
              ]
            }
          ],
          '/en/api/': [
            {
              text: 'API Reference',
              items: [
                { text: 'HTTP REST API', link: '/en/api/http-api' },
                { text: 'gRPC API', link: '/en/api/grpc-api' },
                { text: 'WebAssembly', link: '/en/api/wasm' },
              ]
            }
          ],
          '/en/reference/': [
            {
              text: 'Reference',
              items: [
                { text: 'CLI Options', link: '/en/reference/cli' },
                { text: 'Configuration', link: '/en/reference/configuration' },
                { text: 'Metrics', link: '/en/reference/metrics' },
              ]
            }
          ]
        },
        footer: {
          message: 'Released under the MIT License.',
          copyright: 'Copyright © 2024-present Ordo Contributors'
        },
        editLink: {
          pattern: 'https://github.com/Pama-Lee/Ordo/edit/main/ordo-editor/apps/docs/:path',
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
      description: "高性能规则引擎与可视化编辑器",
      themeConfig: {
        nav: [
          { text: '指南', link: '/zh/guide/getting-started' },
          { text: 'API', link: '/zh/api/http-api' },
          { text: '参考', link: '/zh/reference/cli' },
          { 
            text: '演练场', 
            link: 'https://pama-lee.github.io/Ordo/',
            target: '_self'
          },
        ],
        sidebar: {
          '/zh/guide/': [
            {
              text: '介绍',
              items: [
                { text: 'Ordo 是什么？', link: '/zh/guide/what-is-ordo' },
                { text: '开始使用', link: '/zh/guide/getting-started' },
                { text: '快速入门', link: '/zh/guide/quick-start' },
              ]
            },
            {
              text: '核心概念',
              items: [
                { text: '规则结构', link: '/zh/guide/rule-structure' },
                { text: '表达式语法', link: '/zh/guide/expression-syntax' },
                { text: '内置函数', link: '/zh/guide/builtin-functions' },
              ]
            },
            {
              text: '功能特性',
              items: [
                { text: '规则持久化', link: '/zh/guide/persistence' },
                { text: '版本管理', link: '/zh/guide/versioning' },
                { text: '审计日志', link: '/zh/guide/audit-logging' },
              ]
            }
          ],
          '/zh/api/': [
            {
              text: 'API 参考',
              items: [
                { text: 'HTTP REST API', link: '/zh/api/http-api' },
                { text: 'gRPC API', link: '/zh/api/grpc-api' },
                { text: 'WebAssembly', link: '/zh/api/wasm' },
              ]
            }
          ],
          '/zh/reference/': [
            {
              text: '参考',
              items: [
                { text: 'CLI 选项', link: '/zh/reference/cli' },
                { text: '配置', link: '/zh/reference/configuration' },
                { text: '指标', link: '/zh/reference/metrics' },
              ]
            }
          ]
        },
        footer: {
          message: '基于 MIT 许可发布。',
          copyright: '版权所有 © 2024-present Ordo 贡献者'
        },
        editLink: {
            pattern: 'https://github.com/Pama-Lee/Ordo/edit/main/ordo-editor/apps/docs/:path',
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
    }
  },
  
  themeConfig: {
    // Logo - use small favicon icon
    logo: '/favicon-32x32.png',
    
    // Social links
    socialLinks: [
      { icon: 'github', link: 'https://github.com/Pama-Lee/Ordo' }
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
  }
})
