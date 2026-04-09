import { createI18n } from 'vue-i18n'
import en from './locales/en'
import zhCN from './locales/zh-CN'
import zhTW from './locales/zh-TW'

export type Locale = 'en' | 'zh-CN' | 'zh-TW'

const STORAGE_KEY = 'ordo-locale'

export const LOCALE_OPTIONS: { label: string; value: Locale }[] = [
  { label: 'EN', value: 'en' },
  { label: '简中', value: 'zh-CN' },
  { label: '繁中', value: 'zh-TW' },
]

function detectLocale(): Locale {
  const saved = localStorage.getItem(STORAGE_KEY) as Locale | null
  if (saved && ['en', 'zh-CN', 'zh-TW'].includes(saved)) return saved
  const browser = navigator.language
  if (browser.startsWith('zh-TW') || browser.startsWith('zh-HK')) return 'zh-TW'
  if (browser.startsWith('zh')) return 'zh-CN'
  return 'en'
}

export const i18n = createI18n({
  legacy: false,
  locale: detectLocale(),
  fallbackLocale: 'en',
  messages: {
    en,
    'zh-CN': zhCN,
    'zh-TW': zhTW,
  },
})

export function setLocale(locale: Locale) {
  ;(i18n.global.locale as any).value = locale
  localStorage.setItem(STORAGE_KEY, locale)
  document.documentElement.lang = locale
}
