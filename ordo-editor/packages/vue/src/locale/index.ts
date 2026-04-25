import { ref, inject, type InjectionKey, type Ref } from 'vue';

export type Lang = 'en' | 'zh-CN' | 'zh-TW';

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
    field: string;
    value: string;
    none: string;
    variable: string;
    deleteStep: string;
    endFlow: string;
  };
  condition: {
    selectField: string;
    selectOperator: string;
    enterValue: string;
    switchToExpression: string;
    switchToSmart: string;
    fieldReference: string;
    literalValue: string;
  };
  valueInput: {
    true: string;
    false: string;
    null: string;
  };
  table: {
    addRow: string;
    addInputColumn: string;
    addOutputColumn: string;
    deleteRow: string;
    deleteColumn: string;
    duplicateRow: string;
    hitPolicy: string;
    hitPolicyFirst: string;
    hitPolicyAll: string;
    hitPolicyCollect: string;
    importFromSchema: string;
    exportJson: string;
    validate: string;
    resultCode: string;
    resultMessage: string;
    noRows: string;
    noColumns: string;
    cellExact: string;
    cellRange: string;
    cellList: string;
    cellAny: string;
    cellExpression: string;
    inputColumns: string;
    outputColumns: string;
    priority: string;
    columnField: string;
    columnLabel: string;
    columnType: string;
    addColumn: string;
    showAsFlow: string;
    groupInput: string;
    groupOutput: string;
    groupResult: string;
    traceStatus: string;
    traceInputRow: string;
    traceMatched: string;
    traceNotMatched: string;
    traceUnknown: string;
    traceAllMatched: string;
    traceAnyCell: string;
    traceExprCell: string;
    traceActual: string;
  };
  step: {
    decision: string;
    action: string;
    terminal: string;
    subRule: string;
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
    typeSubRule: string;
    refName: string;
    managedAsset: string;
    customSubRuleName: string;
    assetScope: string;
    assetVersion: string;
    scopeProject: string;
    scopeOrg: string;
    latestPublished: string;
    bindings: string;
    addBinding: string;
    outputs: string;
    addOutput: string;
    childField: string;
    parentVariable: string;
    childVariable: string;
    noBindings: string;
    noSubRuleOutputs: string;
    openSubRuleEditor: string;
    branch: string;
    default: string;
    next: string;
    branchLabel: string;
    ifLabel: string;
    thenLabel: string;
    noBranches: string;
    noAssignments: string;
    noOutputFields: string;
    logMessage: string;
    messageExpr: string;
    codePlaceholder: string;
    logLevelDebug: string;
    logLevelInfo: string;
    logLevelWarn: string;
    logLevelError: string;
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
    newGroup: string;
    externalCall: string;
    conditionLabel: string;
    extractSubRule: string;
    extractSubRuleSelectNodes: string;
    extractSubRuleNoGroups: string;
    extractSubRuleSingleEntry: string;
    extractSubRuleConnected: string;
    extractSubRuleSingleExit: string;
    extractSubRuleNoMixedExit: string;
    extractSubRuleNeedsExit: string;
    extractedSubRuleDescription: string;
    subRuleReturn: string;
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
    field: 'Field',
    value: 'Value',
    none: 'None',
    variable: 'variable',
    deleteStep: 'Delete step',
    endFlow: '-- End Flow --',
  },
  condition: {
    selectField: 'Select field',
    selectOperator: 'Select operator',
    enterValue: 'Enter value',
    switchToExpression: 'Switch to Expression',
    switchToSmart: 'Switch to Smart',
    fieldReference: 'Field Reference',
    literalValue: 'Literal Value',
  },
  valueInput: {
    true: 'true',
    false: 'false',
    null: 'null',
  },
  table: {
    addRow: 'Add Row',
    addInputColumn: 'Add Input Column',
    addOutputColumn: 'Add Output Column',
    deleteRow: 'Delete Row',
    deleteColumn: 'Delete Column',
    duplicateRow: 'Duplicate Row',
    hitPolicy: 'Hit Policy',
    hitPolicyFirst: 'First Match',
    hitPolicyAll: 'All Matches',
    hitPolicyCollect: 'Collect',
    importFromSchema: 'Import from Schema',
    exportJson: 'Export JSON',
    validate: 'Validate',
    resultCode: 'Result Code',
    resultMessage: 'Result Message',
    noRows: 'No rules defined. Click "Add Row" to create the first rule.',
    noColumns: 'No columns defined. Add input and output columns to get started.',
    cellExact: 'Exact',
    cellRange: 'Range',
    cellList: 'List',
    cellAny: 'Any',
    cellExpression: 'Expression',
    inputColumns: 'Input Columns',
    outputColumns: 'Output Columns',
    priority: 'Priority',
    columnField: 'Field Path',
    columnLabel: 'Label',
    columnType: 'Type',
    addColumn: 'Add Column',
    showAsFlow: 'Show as Flow',
    groupInput: 'Conditions',
    groupOutput: 'Outputs',
    groupResult: 'Result',
    traceStatus: 'Trace',
    traceInputRow: 'Input',
    traceMatched: 'Matched',
    traceNotMatched: 'Skipped',
    traceUnknown: 'Unknown',
    traceAllMatched: 'All conditions matched',
    traceAnyCell: 'Wildcard matched',
    traceExprCell: 'Expression cell is not evaluated inline',
    traceActual: 'Actual',
  },
  step: {
    decision: 'Decision',
    action: 'Action',
    terminal: 'Terminal',
    subRule: 'Sub-rule',
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
    typeSubRule: 'Sub-rule',
    refName: 'Reference',
    managedAsset: 'Managed Asset',
    customSubRuleName: 'Custom / Quick Create',
    assetScope: 'Asset Scope',
    assetVersion: 'Asset Version',
    scopeProject: 'Project',
    scopeOrg: 'Organization',
    latestPublished: 'Latest published',
    bindings: 'Input Bindings',
    addBinding: 'Add Binding',
    outputs: 'Output Mappings',
    addOutput: 'Add Output',
    childField: 'Child Field',
    parentVariable: 'Parent Variable',
    childVariable: 'Child Variable',
    noBindings: 'No input bindings.',
    noSubRuleOutputs: 'No output mappings.',
    openSubRuleEditor: 'Open Sub-Rule in Editor',
    branch: 'Branch',
    default: 'Default',
    next: 'Next',
    branchLabel: 'Branch Label',
    ifLabel: 'If',
    thenLabel: 'Then',
    noBranches: 'No branches defined.',
    noAssignments: 'No variable assignments.',
    noOutputFields: 'No output fields.',
    logMessage: 'Log message...',
    messageExpr: 'Message expression...',
    codePlaceholder: 'CODE',
    logLevelDebug: 'Debug',
    logLevelInfo: 'Info',
    logLevelWarn: 'Warn',
    logLevelError: 'Error',
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
    newGroup: 'New Group',
    externalCall: 'Call',
    conditionLabel: 'Condition',
    extractSubRule: 'Extract SubRule',
    extractSubRuleSelectNodes: 'Select one or more steps first.',
    extractSubRuleNoGroups: 'Only step nodes can be extracted. Groups are not supported.',
    extractSubRuleSingleEntry: 'Selection must have exactly one entry point.',
    extractSubRuleConnected: 'Selection must be one connected executable region.',
    extractSubRuleSingleExit: 'Selection can only continue to one outside target.',
    extractSubRuleNoMixedExit:
      'Selection cannot mix terminal endings with paths that continue outside the selection.',
    extractSubRuleNeedsExit: 'Selection must end in a terminal or continue to an outside step.',
    extractedSubRuleDescription: 'Extracted reusable SubRule decision graph.',
    subRuleReturn: 'Return to parent',
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
    field: '字段',
    value: '值',
    none: '无',
    variable: '变量',
    deleteStep: '删除步骤',
    endFlow: '-- 结束流程 --',
  },
  condition: {
    selectField: '选择字段',
    selectOperator: '选择运算符',
    enterValue: '输入值',
    switchToExpression: '切换到表达式',
    switchToSmart: '切换到智能模式',
    fieldReference: '字段引用',
    literalValue: '字面量',
  },
  valueInput: {
    true: '真',
    false: '假',
    null: '空',
  },
  table: {
    addRow: '添加行',
    addInputColumn: '添加输入列',
    addOutputColumn: '添加输出列',
    deleteRow: '删除行',
    deleteColumn: '删除列',
    duplicateRow: '复制行',
    hitPolicy: '命中策略',
    hitPolicyFirst: '首次匹配',
    hitPolicyAll: '全部匹配',
    hitPolicyCollect: '收集',
    importFromSchema: '从 Schema 导入',
    exportJson: '导出 JSON',
    validate: '验证',
    resultCode: '结果码',
    resultMessage: '结果消息',
    noRows: '暂无规则。点击"添加行"创建第一条规则。',
    noColumns: '暂无列定义。请添加输入列和输出列以开始。',
    cellExact: '精确',
    cellRange: '范围',
    cellList: '列表',
    cellAny: '任意',
    cellExpression: '表达式',
    inputColumns: '输入列',
    outputColumns: '输出列',
    priority: '优先级',
    columnField: '字段路径',
    columnLabel: '标签',
    columnType: '类型',
    addColumn: '添加列',
    showAsFlow: '显示为流程图',
    groupInput: '条件',
    groupOutput: '输出',
    groupResult: '结果',
    traceStatus: '追踪',
    traceInputRow: '输入',
    traceMatched: '命中',
    traceNotMatched: '未命中',
    traceUnknown: '未知',
    traceAllMatched: '全部条件命中',
    traceAnyCell: '通配条件命中',
    traceExprCell: '表达式单元格暂不做行内求值',
    traceActual: '实际值',
  },
  step: {
    decision: '决策节点',
    action: '动作节点',
    terminal: '结束节点',
    subRule: '子规则节点',
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
    typeSubRule: '子规则',
    refName: '引用子规则',
    managedAsset: '托管资产',
    customSubRuleName: '自定义 / 快速创建',
    assetScope: '资产范围',
    assetVersion: '资产版本',
    scopeProject: '项目级',
    scopeOrg: '组织级',
    latestPublished: '最新已发布版本',
    bindings: '输入绑定',
    addBinding: '添加绑定',
    outputs: '输出映射',
    addOutput: '添加输出',
    childField: '子规则字段',
    parentVariable: '父级变量',
    childVariable: '子规则变量',
    noBindings: '暂无输入绑定。',
    noSubRuleOutputs: '暂无输出映射。',
    openSubRuleEditor: '在编辑器中打开子规则',
    branch: '分支',
    default: '默认',
    next: '下一步',
    branchLabel: '分支标签',
    ifLabel: '如果',
    thenLabel: '则',
    noBranches: '暂无分支条件。',
    noAssignments: '暂无变量赋值。',
    noOutputFields: '暂无输出字段。',
    logMessage: '日志消息...',
    messageExpr: '消息表达式...',
    codePlaceholder: '返回码',
    logLevelDebug: '调试',
    logLevelInfo: '信息',
    logLevelWarn: '警告',
    logLevelError: '错误',
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
    newGroup: '新分组',
    externalCall: '调用',
    conditionLabel: '条件',
    extractSubRule: '提取为子规则',
    extractSubRuleSelectNodes: '请先选择一个或多个步骤。',
    extractSubRuleNoGroups: '只能提取步骤节点，暂不支持分组节点。',
    extractSubRuleSingleEntry: '选区必须只有一个入口。',
    extractSubRuleConnected: '选区必须是一个连通的可执行区域。',
    extractSubRuleSingleExit: '选区只能继续到一个外部目标。',
    extractSubRuleNoMixedExit: '选区不能同时包含终结路径和继续到外部的路径。',
    extractSubRuleNeedsExit: '选区必须以终结节点结束，或继续到一个外部步骤。',
    extractedSubRuleDescription: '从父规则提取出的可复用子规则决策图。',
    subRuleReturn: '返回父规则',
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

const zhTW: LocaleMessages = {
  common: {
    add: '新增',
    delete: '刪除',
    save: '儲存',
    cancel: '取消',
    confirm: '確認',
    edit: '編輯',
    search: '搜尋...',
    description: '描述',
    name: '名稱',
    version: '版本',
    optional: '可選',
    unknown: '未知',
    close: '關閉',
    field: '欄位',
    value: '值',
    none: '無',
    variable: '變數',
    deleteStep: '刪除步驟',
    endFlow: '-- 結束流程 --',
  },
  condition: {
    selectField: '選擇欄位',
    selectOperator: '選擇運算子',
    enterValue: '輸入值',
    switchToExpression: '切換至運算式',
    switchToSmart: '切換至智慧模式',
    fieldReference: '欄位參照',
    literalValue: '字面值',
  },
  valueInput: {
    true: '真',
    false: '假',
    null: '空',
  },
  table: {
    addRow: '新增列',
    addInputColumn: '新增輸入欄',
    addOutputColumn: '新增輸出欄',
    deleteRow: '刪除列',
    deleteColumn: '刪除欄',
    duplicateRow: '複製列',
    hitPolicy: '命中策略',
    hitPolicyFirst: '首次匹配',
    hitPolicyAll: '全部匹配',
    hitPolicyCollect: '收集',
    importFromSchema: '從 Schema 匯入',
    exportJson: '匯出 JSON',
    validate: '驗證',
    resultCode: '結果碼',
    resultMessage: '結果訊息',
    noRows: '尚無規則。點擊「新增列」建立第一條規則。',
    noColumns: '尚無欄位定義。請新增輸入欄和輸出欄以開始。',
    cellExact: '精確',
    cellRange: '範圍',
    cellList: '清單',
    cellAny: '任意',
    cellExpression: '運算式',
    inputColumns: '輸入欄',
    outputColumns: '輸出欄',
    priority: '優先順序',
    columnField: '欄位路徑',
    columnLabel: '標籤',
    columnType: '類型',
    addColumn: '新增欄',
    showAsFlow: '顯示為流程圖',
    groupInput: '條件',
    groupOutput: '輸出',
    groupResult: '結果',
    traceStatus: '追蹤',
    traceInputRow: '輸入',
    traceMatched: '命中',
    traceNotMatched: '未命中',
    traceUnknown: '未知',
    traceAllMatched: '全部條件命中',
    traceAnyCell: '萬用條件命中',
    traceExprCell: '表達式儲存格暫不做行內求值',
    traceActual: '實際值',
  },
  step: {
    decision: '決策節點',
    action: '動作節點',
    terminal: '結束節點',
    subRule: '子規則節點',
    start: '起始',
    setAsStart: '設為起始',
    branches: '分支條件',
    nextStep: '下一步',
    defaultNext: '預設分支 (Else)',
    addBranch: '新增分支',
    assignments: '變數賦值',
    addAssignment: '新增變數',
    logging: '日誌記錄',
    resultCode: '回傳碼',
    resultMessage: '回傳訊息',
    outputFields: '輸出欄位',
    typeDecision: '決策',
    typeAction: '動作',
    typeTerminal: '終結',
    typeSubRule: '子規則',
    refName: '引用子規則',
    managedAsset: '託管資產',
    customSubRuleName: '自訂 / 快速建立',
    assetScope: '資產範圍',
    assetVersion: '資產版本',
    scopeProject: '專案級',
    scopeOrg: '組織級',
    latestPublished: '最新已發佈版本',
    bindings: '輸入綁定',
    addBinding: '新增綁定',
    outputs: '輸出映射',
    addOutput: '新增輸出',
    childField: '子規則欄位',
    parentVariable: '父級變數',
    childVariable: '子規則變數',
    noBindings: '尚無輸入綁定。',
    noSubRuleOutputs: '尚無輸出映射。',
    openSubRuleEditor: '在編輯器中開啟子規則',
    branch: '分支',
    default: '預設',
    next: '下一步',
    branchLabel: '分支標籤',
    ifLabel: '如果',
    thenLabel: '則',
    noBranches: '尚無分支條件。',
    noAssignments: '尚無變數賦值。',
    noOutputFields: '尚無輸出欄位。',
    logMessage: '日誌訊息...',
    messageExpr: '訊息運算式...',
    codePlaceholder: '回傳碼',
    logLevelDebug: '除錯',
    logLevelInfo: '資訊',
    logLevelWarn: '警告',
    logLevelError: '錯誤',
  },
  flow: {
    createGroup: '建立群組',
    setAsStart: '設為起始',
    duplicate: '複製',
    group: '群組',
    ungroupNodes: '取消群組',
    reverseEdge: '反轉方向',
    deleteEdge: '刪除連線',
    add: '新增',
    layout: '佈局',
    autoLayout: '自動佈局',
    auto: '自動',
    edge: '連線',
    bezier: '貝茲',
    step: '階梯',
    direction: '方向',
    lr: '左 → 右',
    tb: '上 → 下',
    rl: '右 → 左',
    bt: '下 → 上',
    deleteSelected: '刪除所選',
    stepsInGroup: '群組內步驟',
    deleteGroup: '刪除群組',
    groupDropZone: '拖曳節點至此處，或右鍵點擊所選節點來建立群組',
    steps: '個步驟',
    ungroupedSteps: '未分組步驟',
    allSteps: '所有步驟',
    moveTo: '移至...',
    noSteps: '此階段尚無步驟',
    noStepsYet: '尚無步驟。請新增步驟以開始。',
    newGroup: '新群組',
    externalCall: '呼叫',
    conditionLabel: '條件',
    extractSubRule: '提取為子規則',
    extractSubRuleSelectNodes: '請先選擇一個或多個步驟。',
    extractSubRuleNoGroups: '只能提取步驟節點，暫不支援群組節點。',
    extractSubRuleSingleEntry: '選區必須只有一個入口。',
    extractSubRuleConnected: '選區必須是一個連通的可執行區域。',
    extractSubRuleSingleExit: '選區只能繼續到一個外部目標。',
    extractSubRuleNoMixedExit: '選區不能同時包含終結路徑和繼續到外部的路徑。',
    extractSubRuleNeedsExit: '選區必須以終結節點結束，或繼續到一個外部步驟。',
    extractedSubRuleDescription: '從父規則提取出的可重用子規則決策圖。',
    subRuleReturn: '返回父規則',
  },
  validation: {
    valid: '有效',
    invalid: '無效',
    passed: '驗證通過',
    failed: '驗證失敗',
  },
  execution: {
    title: '執行規則',
    input: '輸入資料',
    inputPlaceholder: '輸入 JSON 格式的資料...',
    mode: '執行模式',
    modeWasm: '本機執行 (WASM)',
    modeHttp: '遠端執行 (HTTP)',
    httpEndpoint: 'HTTP 端點',
    execute: '執行',
    executing: '執行中...',
    includeTrace: '包含執行軌跡',
    result: '執行結果',
    trace: '執行軌跡',
    error: '錯誤',
    duration: '耗時',
    code: '結果碼',
    message: '訊息',
    output: '輸出',
    path: '路徑',
    stepId: '步驟 ID',
    stepName: '步驟名稱',
    stepDuration: '耗時',
    parseError: 'JSON 解析失敗',
    executionError: '執行失敗',
    compatibilityError: '相容性錯誤',
    noResult: '尚無執行結果。點擊「執行」按鈕運行規則。',
    noTrace: '尚無執行軌跡。請在執行前啟用「Trace」選項。',
    history: '歷史紀錄',
    noHistory: '尚無執行歷史。',
    clearHistory: '清空歷史',
    loadSample: '載入範例',
    showInFlow: '在流程圖中顯示',
    hideFromFlow: '隱藏流程圖追蹤',
  },
};

const messages: Record<Lang, LocaleMessages> = {
  en,
  'zh-CN': zhCN,
  'zh-TW': zhTW,
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
