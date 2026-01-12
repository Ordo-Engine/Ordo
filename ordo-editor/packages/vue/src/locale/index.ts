import { ref, inject, provide, computed, type InjectionKey, type Ref } from 'vue';

export type Lang = 'en' | 'zh-CN';

export interface LocaleMessages {
  common: {
    add: string;
    delete: string;
    save: string;
    cancel: string;
    confirm: string;
    edit: string;
    search: string;
    description: string;
    name: string;
    version: string;
    optional: string;
    unknown: string;
    close: string;
  };
  step: {
    decision: string;
    action: string;
    terminal: string;
    start: string;
    setAsStart: string;
    branches: string;
    nextStep: string;
    defaultNext: string;
    addBranch: string;
    assignments: string;
    addAssignment: string;
    logging: string;
    resultCode: string;
    resultMessage: string;
    outputFields: string;
    typeDecision: string;
    typeAction: string;
    typeTerminal: string;
    branch: string;
    default: string;
    next: string;
  };
  flow: {
    createGroup: string;
    setAsStart: string;
    duplicate: string;
    group: string;
    ungroupNodes: string;
    reverseEdge: string;
    deleteEdge: string;
    add: string;
    layout: string;
    autoLayout: string;
    auto: string;
    edge: string;
    bezier: string;
    step: string;
    direction: string;
    lr: string;
    tb: string;
    rl: string;
    bt: string;
    deleteSelected: string;
    stepsInGroup: string;
    deleteGroup: string;
    groupDropZone: string;
    steps: string;
    ungroupedSteps: string;
    allSteps: string;
    moveTo: string;
    noSteps: string;
    noStepsYet: string;
  };
  validation: {
    valid: string;
    invalid: string;
    passed: string;
    failed: string;
  };
  execution: {
    title: string;
    input: string;
    inputPlaceholder: string;
    mode: string;
    modeWasm: string;
    modeHttp: string;
    httpEndpoint: string;
    execute: string;
    executing: string;
    includeTrace: string;
    result: string;
    trace: string;
    error: string;
    duration: string;
    code: string;
    message: string;
    output: string;
    path: string;
    stepId: string;
    stepName: string;
    stepDuration: string;
    parseError: string;
    executionError: string;
    compatibilityError: string;
    noResult: string;
    noTrace: string;
    history: string;
    noHistory: string;
    clearHistory: string;
    loadSample: string;
    showInFlow: string;
    hideFromFlow: string;
  };
}

const en: LocaleMessages = {
  common: {
    add: 'Add',
    delete: 'Delete',
    save: 'Save',
    cancel: 'Cancel',
    confirm: 'Confirm',
    edit: 'Edit',
    search: 'Search...',
    description: 'Description',
    name: 'Name',
    version: 'Version',
    optional: 'Optional',
    unknown: 'Unknown',
    close: 'Close',
  },
  step: {
    decision: 'Decision',
    action: 'Action',
    terminal: 'Terminal',
    start: 'START',
    setAsStart: 'Set Start',
    branches: 'Branches',
    nextStep: 'Next Step',
    defaultNext: 'Default (Else)',
    addBranch: 'Add Branch',
    assignments: 'Variables',
    addAssignment: 'Add Variable',
    logging: 'Logging',
    resultCode: 'Result Code',
    resultMessage: 'Result Message',
    outputFields: 'Outputs',
    typeDecision: 'Decision',
    typeAction: 'Action',
    typeTerminal: 'Terminal',
    branch: 'Branch',
    default: 'Default',
    next: 'Next',
  },
  flow: {
    createGroup: 'Create Group',
    setAsStart: 'Set as Start',
    duplicate: 'Duplicate',
    group: 'Group',
    ungroupNodes: 'Ungroup',
    reverseEdge: 'Reverse Direction',
    deleteEdge: 'Delete Connection',
    add: 'Add',
    layout: 'Layout',
    autoLayout: 'Auto Layout',
    auto: 'Auto',
    edge: 'Edge',
    bezier: 'Bezier',
    step: 'Step',
    direction: 'Direction',
    lr: 'Left → Right',
    tb: 'Top → Bottom',
    rl: 'Right → Left',
    bt: 'Bottom → Top',
    deleteSelected: 'Delete Selected',
    stepsInGroup: 'Steps in group',
    deleteGroup: 'Delete Group',
    groupDropZone: 'Drag nodes here or select nodes and right-click to group',
    steps: 'steps',
    ungroupedSteps: 'Ungrouped Steps',
    allSteps: 'All Steps',
    moveTo: 'Move to...',
    noSteps: 'No steps in this stage',
    noStepsYet: 'No steps yet. Add a step to get started.',
  },
  validation: {
    valid: 'Valid',
    invalid: 'Invalid',
    passed: 'PASSED',
    failed: 'FAILED',
  },
  execution: {
    title: 'Execute Rule',
    input: 'Input Data',
    inputPlaceholder: 'Enter JSON input data...',
    mode: 'Execution Mode',
    modeWasm: 'Local (WASM)',
    modeHttp: 'Remote (HTTP)',
    httpEndpoint: 'HTTP Endpoint',
    execute: 'Execute',
    executing: 'Executing...',
    includeTrace: 'Include Execution Trace',
    result: 'Result',
    trace: 'Execution Trace',
    error: 'Error',
    duration: 'Duration',
    code: 'Code',
    message: 'Message',
    output: 'Output',
    path: 'Path',
    stepId: 'Step ID',
    stepName: 'Step Name',
    stepDuration: 'Duration',
    parseError: 'Failed to parse input JSON',
    executionError: 'Execution failed',
    compatibilityError: 'Compatibility error',
    noResult: 'No execution result yet. Click "Execute" to run the rule.',
    noTrace: 'No trace available. Enable "Trace" option before execution.',
    history: 'History',
    noHistory: 'No execution history.',
    clearHistory: 'Clear History',
    loadSample: 'Load Sample',
    showInFlow: 'Show in Flow',
    hideFromFlow: 'Hide from Flow',
  },
};

const zhCN: LocaleMessages = {
  common: {
    add: '添加',
    delete: '删除',
    save: '保存',
    cancel: '取消',
    confirm: '确认',
    edit: '编辑',
    search: '搜索...',
    description: '描述',
    name: '名称',
    version: '版本',
    optional: '可选',
    unknown: '未知',
    close: '关闭',
  },
  step: {
    decision: '决策节点',
    action: '动作节点',
    terminal: '结束节点',
    start: '起始',
    setAsStart: '设为起始',
    branches: '分支条件',
    nextStep: '下一步',
    defaultNext: '默认分支 (Else)',
    addBranch: '添加分支',
    assignments: '变量赋值',
    addAssignment: '添加变量',
    logging: '日志记录',
    resultCode: '返回码',
    resultMessage: '返回信息',
    outputFields: '输出字段',
    typeDecision: '决策',
    typeAction: '动作',
    typeTerminal: '终结',
    branch: '分支',
    default: '默认',
    next: '下一步',
  },
  flow: {
    createGroup: '创建分组',
    setAsStart: '设为起始',
    duplicate: '复制',
    group: '分组',
    ungroupNodes: '取消分组',
    reverseEdge: '反转方向',
    deleteEdge: '删除连线',
    add: '添加',
    layout: '布局',
    autoLayout: '自动布局',
    auto: '自动',
    edge: '连线',
    bezier: '贝塞尔',
    step: '阶梯',
    direction: '方向',
    lr: '左 → 右',
    tb: '上 → 下',
    rl: '右 → 左',
    bt: '下 → 上',
    deleteSelected: '删除选中',
    stepsInGroup: '组内步骤',
    deleteGroup: '删除分组',
    groupDropZone: '拖入节点到此处，或右键点击选中的节点来创建分组',
    steps: '个步骤',
    ungroupedSteps: '未分组步骤',
    allSteps: '所有步骤',
    moveTo: '移动到...',
    noSteps: '此阶段暂无步骤',
    noStepsYet: '暂无步骤。请添加步骤开始。',
  },
  validation: {
    valid: '有效',
    invalid: '无效',
    passed: '验证通过',
    failed: '验证失败',
  },
  execution: {
    title: '执行规则',
    input: '输入数据',
    inputPlaceholder: '输入 JSON 格式的数据...',
    mode: '执行模式',
    modeWasm: '本地执行 (WASM)',
    modeHttp: '远程执行 (HTTP)',
    httpEndpoint: 'HTTP 端点',
    execute: '执行',
    executing: '执行中...',
    includeTrace: '包含执行轨迹',
    result: '执行结果',
    trace: '执行轨迹',
    error: '错误',
    duration: '耗时',
    code: '结果码',
    message: '消息',
    output: '输出',
    path: '路径',
    stepId: '步骤 ID',
    stepName: '步骤名称',
    stepDuration: '耗时',
    parseError: 'JSON 解析失败',
    executionError: '执行失败',
    compatibilityError: '兼容性错误',
    noResult: '暂无执行结果。点击"执行"按钮运行规则。',
    noTrace: '暂无执行轨迹。请在执行前启用"Trace"选项。',
    history: '历史记录',
    noHistory: '暂无执行历史。',
    clearHistory: '清空历史',
    loadSample: '加载示例',
    showInFlow: '在流程图中显示',
    hideFromFlow: '隐藏流程图追踪',
  },
};

const messages: Record<Lang, LocaleMessages> = {
  en,
  'zh-CN': zhCN,
};

// Export the key so it can be used by providers
export const LOCALE_KEY: InjectionKey<Ref<Lang>> = Symbol.for('ordo-locale');

export function createI18n(defaultLang: Lang = 'en') {
  const currentLang = ref<Lang>(defaultLang);

  const install = (app: any) => {
    app.provide(LOCALE_KEY, currentLang);
  };

  return {
    currentLang,
    install,
  };
}

export function useI18n() {
  const locale = inject(LOCALE_KEY, ref<Lang>('en'));

  const t = (path: string): string => {
    const keys = path.split('.');
    let current: any = messages[locale.value];

    for (const key of keys) {
      if (current[key] === undefined) return path;
      current = current[key];
    }

    return current;
  };

  return { locale, t };
}
