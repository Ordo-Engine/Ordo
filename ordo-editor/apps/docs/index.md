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

onMounted(() => {
  if (typeof window !== 'undefined') {
    // Avoid redirecting if we are already coming from a back button or if user explicitly navigated here
    // But this is the root.
    const lang = navigator.language.toLowerCase()
    const base = '/Ordo/docs'
    
    // Simple redirect
    if (lang.startsWith('zh')) {
        window.location.replace(`${base}/zh/`)
    } else {
        window.location.replace(`${base}/en/`)
    }
  }
})
</script>
