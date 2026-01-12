import { defineConfig } from 'vitepress'

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
    ['link', { rel: 'icon', href: '/Ordo/docs/favicon.ico' }],
  ],
  
  themeConfig: {
    // Logo
    logo: '/logo.png',
    
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
