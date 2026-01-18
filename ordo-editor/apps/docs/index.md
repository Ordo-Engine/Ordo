---
layout: home
hero:
  name: Ordo
  text: High-Performance Rule Engine
  tagline: Select your language / 选择您的语言
  actions:
    - theme: brand
      text: English
      link: /en/
    - theme: alt
      text: 简体中文
      link: /zh/
---

<script setup>
import { onMounted } from 'vue'
import { useData } from 'vitepress'

onMounted(() => {
  if (typeof window !== 'undefined') {
    const { site } = useData()
    const lang = navigator.language.toLowerCase()
    // Use VitePress base path (already includes trailing slash handling)
    const base = site.value.base.replace(/\/$/, '')
    
    // Simple redirect based on browser language
    if (lang.startsWith('zh')) {
        window.location.replace(`${base}/zh/`)
    } else {
        window.location.replace(`${base}/en/`)
    }
  }
})
</script>
