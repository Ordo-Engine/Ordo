import { ref, computed } from 'vue';

type Locale = 'en' | 'zh-CN';

const messages = {
  en: {
    welcome: {
      title: 'Welcome to Ordo Playground',
      desc: 'Ordo is a high-performance rule engine with a visual editor.<br />Design complex business rules with ease.',
      features: {
        visual: 'Visual Flow Editor',
        wasm: 'WASM-powered Execution',
        trace: 'Step-by-step Tracing',
      },
      actions: {
        tour: 'Take a Quick Tour',
        skip: "Skip, I'll explore myself",
        docs: 'View Documentation',
      },
      hint: 'You can restart the tour anytime from the help button',
    },
    app: {
      theme: 'Toggle Theme',
      lang: 'Switch Language',
      reset: 'Reset',
      help: 'Help / Tour',
      github: 'GitHub',
    },
  },
  'zh-CN': {
    welcome: {
      title: '欢迎使用 Ordo 演练场',
      desc: 'Ordo 是一个带有可视化编辑器的高性能规则引擎。<br />轻松设计复杂的业务规则。',
      features: {
        visual: '可视化流程编辑器',
        wasm: 'WASM 驱动的执行',
        trace: '分步追踪',
      },
      actions: {
        tour: '快速导览',
        skip: '跳过，我自己探索',
        docs: '查看文档',
      },
      hint: '您可以随时从帮助按钮重新开始导览',
    },
    app: {
      theme: '切换主题',
      lang: '切换语言',
      reset: '重置',
      help: '帮助 / 导览',
      github: 'GitHub',
    },
  },
};

const currentLocale = ref<Locale>('en');

export function useI18n() {
  const t = (path: string) => {
    const keys = path.split('.');
    let value: any = messages[currentLocale.value];

    for (const key of keys) {
      if (value && typeof value === 'object' && key in value) {
        value = value[key as keyof typeof value];
      } else {
        return path;
      }
    }

    return value as string;
  };

  const setLocale = (locale: Locale) => {
    currentLocale.value = locale;
  };

  return {
    locale: currentLocale,
    t,
    setLocale,
  };
}
