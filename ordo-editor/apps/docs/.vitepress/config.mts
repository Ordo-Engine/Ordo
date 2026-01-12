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
  
  themeConfig: {
    // Logo - use small favicon icon
    logo: '/favicon-32x32.png',
    
    // Navigation bar
    nav: [
      { text: 'Guide', link: '/guide/getting-started' },
      { text: 'API', link: '/api/http-api' },
      { text: 'Reference', link: '/reference/cli' },
      { 
        text: 'Playground', 
        link: 'https://pama-lee.github.io/Ordo/',
        target: '_self'
      },
    ],

    // Sidebar navigation
    sidebar: {
      '/guide/': [
        {
          text: 'Introduction',
          items: [
            { text: 'What is Ordo?', link: '/guide/what-is-ordo' },
            { text: 'Getting Started', link: '/guide/getting-started' },
            { text: 'Quick Start', link: '/guide/quick-start' },
          ]
        },
        {
          text: 'Core Concepts',
          items: [
            { text: 'Rule Structure', link: '/guide/rule-structure' },
            { text: 'Expression Syntax', link: '/guide/expression-syntax' },
            { text: 'Built-in Functions', link: '/guide/builtin-functions' },
          ]
        },
        {
          text: 'Features',
          items: [
            { text: 'Rule Persistence', link: '/guide/persistence' },
            { text: 'Version Management', link: '/guide/versioning' },
            { text: 'Audit Logging', link: '/guide/audit-logging' },
          ]
        }
      ],
      '/api/': [
        {
          text: 'API Reference',
          items: [
            { text: 'HTTP REST API', link: '/api/http-api' },
            { text: 'gRPC API', link: '/api/grpc-api' },
            { text: 'WebAssembly', link: '/api/wasm' },
          ]
        }
      ],
      '/reference/': [
        {
          text: 'Reference',
          items: [
            { text: 'CLI Options', link: '/reference/cli' },
            { text: 'Configuration', link: '/reference/configuration' },
            { text: 'Metrics', link: '/reference/metrics' },
          ]
        }
      ]
    },

    // Social links
    socialLinks: [
      { icon: 'github', link: 'https://github.com/Pama-Lee/Ordo' }
    ],
    
    // Footer
    footer: {
      message: 'Released under the MIT License.',
      copyright: 'Copyright © 2024-present Ordo Contributors'
    },
    
    // Edit link
    editLink: {
      pattern: 'https://github.com/Pama-Lee/Ordo/edit/main/ordo-editor/apps/docs/:path',
      text: 'Edit this page on GitHub'
    },
    
    // Search
    search: {
      provider: 'local'
    },
    
    // Outline
    outline: {
      level: [2, 3],
      label: 'On this page'
    }
  }
})
