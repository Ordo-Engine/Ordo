<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue';
import {
  OrdoFormEditor,
  OrdoFlowEditor,
  OrdoExecutionPanel,
  serializeRuleSet,
  OrdoIcon,
  type RuleSet,
  type SchemaField,
  type Lang,
} from '@ordo/editor-vue';
import { Step, Condition, Expr, generateId } from '@ordo/editor-core';

// Theme & Locale & Editor Mode
const theme = ref<'light' | 'dark'>('dark');
const locale = ref<Lang>('en');
const editorMode = ref<'form' | 'flow'>('form');

// Sidebar visibility
const showLeftSidebar = ref(true);
const showRightSidebar = ref(true);

// Sidebar widths (resizable)
const leftSidebarWidth = ref(240);
const rightSidebarWidth = ref(360);

// Resizing state
const isResizingLeft = ref(false);
const isResizingRight = ref(false);

function toggleTheme() {
  theme.value = theme.value === 'light' ? 'dark' : 'light';
  document.documentElement.setAttribute('data-ordo-theme', theme.value);
}

function toggleLocale() {
  locale.value = locale.value === 'en' ? 'zh-CN' : 'en';
}

function setEditorMode(mode: 'form' | 'flow') {
  editorMode.value = mode;
}

function toggleLeftSidebar() {
  showLeftSidebar.value = !showLeftSidebar.value;
}

function toggleRightSidebar() {
  showRightSidebar.value = !showRightSidebar.value;
}

// Execution panel
const showExecutionPanel = ref(false);
const executionPanelHeight = ref(300);

// Execution trace for flow overlay
const executionTrace = ref<any>(null);

function toggleExecutionPanel() {
  showExecutionPanel.value = !showExecutionPanel.value;
}

function onShowInFlow(trace: any) {
  executionTrace.value = trace;
  // Switch to flow mode if not already
  if (editorMode.value !== 'flow') {
    editorMode.value = 'flow';
  }
}

function onClearFlowTrace() {
  executionTrace.value = null;
}

// Sample input data for each ruleset
const sampleInputData: Record<string, string> = {
  'file_1': JSON.stringify({
    user: {
      id: 'user_001',
      age: 28,
      level: 'vip',
      balance: 5000,
      registeredDays: 365
    },
    order: {
      amount: 500,
      type: 'purchase',
      channel: 'web',
      itemCount: 3
    },
    context: {
      ip: '192.168.1.1',
      device: 'desktop',
      time: new Date().toISOString()
    }
  }, null, 2),
  'file_2': JSON.stringify({
    transaction: {
      amount: 15000,
      currency: 'USD',
      type: 'transfer',
      merchantId: 'merchant_001'
    },
    user: {
      id: 'user_002',
      riskScore: 35,
      verifiedLevel: 2,
      failedAttempts: 0
    },
    device: {
      fingerprint: 'fp_abc123',
      isNewDevice: false,
      location: 'New York, US'
    }
  }, null, 2),
  'file_3': JSON.stringify({
    customer: {
      id: 'cust_001',
      tier: 'gold',
      totalPurchases: 15000,
      isNewCustomer: false
    },
    cart: {
      subtotal: 650,
      itemCount: 5,
      categories: ['electronics', 'clothing'],
      hasCoupon: false
    },
    promotion: {
      campaignId: 'summer_sale_2024',
      isHoliday: true
    }
  }, null, 2),
};

// Get current sample input
const currentSampleInput = computed(() => {
  return sampleInputData[activeFileId.value] || '{\n  \n}';
});

// Resize handlers
function startResizeLeft(e: MouseEvent) {
  isResizingLeft.value = true;
  e.preventDefault();
}

function startResizeRight(e: MouseEvent) {
  isResizingRight.value = true;
  e.preventDefault();
}

function handleMouseMove(e: MouseEvent) {
  if (isResizingLeft.value) {
    const newWidth = e.clientX - 48;
    leftSidebarWidth.value = Math.max(180, Math.min(400, newWidth));
  }
  if (isResizingRight.value) {
    const newWidth = window.innerWidth - e.clientX;
    rightSidebarWidth.value = Math.max(200, Math.min(600, newWidth));
  }
}

function handleMouseUp() {
  isResizingLeft.value = false;
  isResizingRight.value = false;
}

onMounted(() => {
  document.addEventListener('mousemove', handleMouseMove);
  document.addEventListener('mouseup', handleMouseUp);
});

onUnmounted(() => {
  document.removeEventListener('mousemove', handleMouseMove);
  document.removeEventListener('mouseup', handleMouseUp);
});

// ============ Multi-file Management ============

interface OrdoFile {
  id: string;
  name: string;
  ruleset: RuleSet;
  modified: boolean;
}

// Sample schemas for different rule types
const paymentSchema: SchemaField[] = [
  { name: 'user', type: 'object', description: 'User information', fields: [
    { name: 'id', type: 'string', required: true, description: 'User ID' },
    { name: 'age', type: 'number', description: 'User age' },
    { name: 'level', type: 'string', description: 'VIP level (normal/silver/gold/vip)' },
    { name: 'balance', type: 'number', description: 'Account balance' },
    { name: 'registeredDays', type: 'number', description: 'Days since registration' },
  ]},
  { name: 'order', type: 'object', description: 'Order information', fields: [
    { name: 'amount', type: 'number', required: true, description: 'Order amount' },
    { name: 'type', type: 'string', description: 'Order type' },
    { name: 'channel', type: 'string', description: 'Payment channel' },
    { name: 'itemCount', type: 'number', description: 'Number of items' },
  ]},
  { name: 'context', type: 'object', description: 'Request context', fields: [
    { name: 'ip', type: 'string', description: 'Client IP' },
    { name: 'device', type: 'string', description: 'Device type' },
    { name: 'time', type: 'string', description: 'Request time' },
  ]},
];

const riskSchema: SchemaField[] = [
  { name: 'transaction', type: 'object', description: 'Transaction data', fields: [
    { name: 'amount', type: 'number', required: true, description: 'Transaction amount' },
    { name: 'currency', type: 'string', description: 'Currency code' },
    { name: 'type', type: 'string', description: 'Transaction type' },
    { name: 'merchantId', type: 'string', description: 'Merchant ID' },
  ]},
  { name: 'user', type: 'object', description: 'User profile', fields: [
    { name: 'id', type: 'string', required: true, description: 'User ID' },
    { name: 'riskScore', type: 'number', description: 'Historical risk score (0-100)' },
    { name: 'verifiedLevel', type: 'number', description: 'KYC verification level' },
    { name: 'failedAttempts', type: 'number', description: 'Recent failed attempts' },
  ]},
  { name: 'device', type: 'object', description: 'Device info', fields: [
    { name: 'fingerprint', type: 'string', description: 'Device fingerprint' },
    { name: 'isNewDevice', type: 'boolean', description: 'Is new device' },
    { name: 'location', type: 'string', description: 'Geo location' },
  ]},
];

const discountSchema: SchemaField[] = [
  { name: 'customer', type: 'object', description: 'Customer info', fields: [
    { name: 'id', type: 'string', required: true, description: 'Customer ID' },
    { name: 'tier', type: 'string', description: 'Membership tier' },
    { name: 'totalPurchases', type: 'number', description: 'Total historical purchases' },
    { name: 'isNewCustomer', type: 'boolean', description: 'First time buyer' },
  ]},
  { name: 'cart', type: 'object', description: 'Shopping cart', fields: [
    { name: 'subtotal', type: 'number', required: true, description: 'Cart subtotal' },
    { name: 'itemCount', type: 'number', description: 'Number of items' },
    { name: 'categories', type: 'array', description: 'Item categories' },
    { name: 'hasCoupon', type: 'boolean', description: 'Has coupon applied' },
  ]},
  { name: 'promotion', type: 'object', description: 'Current promotions', fields: [
    { name: 'campaignId', type: 'string', description: 'Active campaign ID' },
    { name: 'isHoliday', type: 'boolean', description: 'Is holiday period' },
  ]},
];

// Sample rulesets
const paymentRuleset: RuleSet = {
  config: {
    name: 'Payment Validation',
    version: '1.0.0',
    description: 'Validate payment requests based on user and order information',
    inputSchema: paymentSchema,
    enableTrace: true,
  },
  startStepId: 'step_check_user',
  steps: [
    Step.decision({
      id: 'step_check_user',
      name: 'Check User Level',
      description: 'Route based on user VIP level',
      branches: [
        Step.branch({
          id: 'branch_vip',
          label: 'VIP User',
          condition: Condition.simple(
            Expr.variable('$.user.level'),
            'eq',
            Expr.string('vip')
          ),
          nextStepId: 'step_vip_discount',
        }),
        Step.branch({
          id: 'branch_gold',
          label: 'Gold User',
          condition: Condition.simple(
            Expr.variable('$.user.level'),
            'eq',
            Expr.string('gold')
          ),
          nextStepId: 'step_gold_discount',
        }),
      ],
      defaultNextStepId: 'step_check_amount',
    }),
    Step.action({
      id: 'step_vip_discount',
      name: 'Apply VIP Discount',
      description: 'Apply 20% discount for VIP users',
      assignments: [
        { name: 'discountRate', value: Expr.number(0.2) },
        { name: 'userTier', value: Expr.string('VIP') },
      ],
      logging: {
        message: Expr.string('VIP discount applied: 20%'),
        level: 'info',
      },
      nextStepId: 'step_approve',
    }),
    Step.action({
      id: 'step_gold_discount',
      name: 'Apply Gold Discount',
      description: 'Apply 10% discount for Gold users',
      assignments: [
        { name: 'discountRate', value: Expr.number(0.1) },
        { name: 'userTier', value: Expr.string('Gold') },
      ],
      nextStepId: 'step_check_amount',
    }),
    Step.decision({
      id: 'step_check_amount',
      name: 'Check Order Amount',
      description: 'Validate order amount limits',
      branches: [
        Step.branch({
          id: 'branch_high_amount',
          label: 'High Amount (>10000)',
          condition: Condition.simple(
            Expr.variable('$.order.amount'),
            'gt',
            Expr.number(10000)
          ),
          nextStepId: 'step_manual_review',
        }),
        Step.branch({
          id: 'branch_low_amount',
          label: 'Low Amount (<100)',
          condition: Condition.simple(
            Expr.variable('$.order.amount'),
            'lt',
            Expr.number(100)
          ),
          nextStepId: 'step_fast_approve',
        }),
      ],
      defaultNextStepId: 'step_approve',
    }),
    Step.terminal({
      id: 'step_fast_approve',
      name: 'Fast Approve',
      code: 'FAST_APPROVED',
      message: Expr.string('Small amount fast approved'),
      output: [
        { name: 'allowed', value: Expr.boolean(true) },
        { name: 'reviewRequired', value: Expr.boolean(false) },
      ],
    }),
    Step.terminal({
      id: 'step_approve',
      name: 'Approve Payment',
      code: 'APPROVED',
      message: Expr.string('Payment approved'),
      output: [
        { name: 'allowed', value: Expr.boolean(true) },
        { name: 'message', value: Expr.string('Payment is allowed') },
      ],
    }),
    Step.terminal({
      id: 'step_manual_review',
      name: 'Manual Review Required',
      code: 'PENDING_REVIEW',
      message: Expr.string('High amount requires manual review'),
      output: [
        { name: 'allowed', value: Expr.boolean(false) },
        { name: 'reviewRequired', value: Expr.boolean(true) },
        { name: 'reason', value: Expr.string('Amount exceeds auto-approval limit') },
      ],
    }),
  ],
  groups: [
    {
      id: 'stage_1',
      name: 'Stage 1: User Verification',
      description: 'Verify user identity and apply user-level discounts',
      color: '#1e3a5f',
      position: { x: 0, y: 0 },
      size: { width: 400, height: 300 },
      stepIds: ['step_check_user', 'step_vip_discount', 'step_gold_discount'],
    },
    {
      id: 'stage_2',
      name: 'Stage 2: Amount Validation',
      description: 'Check order amount and apply appropriate rules',
      color: '#4d3319',
      position: { x: 0, y: 0 },
      size: { width: 400, height: 300 },
      stepIds: ['step_check_amount'],
    },
    {
      id: 'stage_3',
      name: 'Stage 3: Result Output',
      description: 'Final decision and output generation',
      color: '#1e4d2b',
      position: { x: 0, y: 0 },
      size: { width: 400, height: 300 },
      stepIds: ['step_fast_approve', 'step_approve', 'step_manual_review'],
    },
  ],
  metadata: {
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  },
};

const riskRuleset: RuleSet = {
  config: {
    name: 'Risk Assessment',
    version: '1.0.0',
    description: 'Evaluate transaction risk level',
    inputSchema: riskSchema,
    enableTrace: true,
  },
  startStepId: 'step_check_amount_risk',
  steps: [
    Step.decision({
      id: 'step_check_amount_risk',
      name: 'Check Transaction Amount',
      description: 'Initial amount-based risk check',
      branches: [
        Step.branch({
          id: 'branch_very_high',
          label: 'Very High Amount (>50000)',
          condition: Condition.simple(
            Expr.variable('$.transaction.amount'),
            'gt',
            Expr.number(50000)
          ),
          nextStepId: 'step_block',
        }),
        Step.branch({
          id: 'branch_high',
          label: 'High Amount (>10000)',
          condition: Condition.simple(
            Expr.variable('$.transaction.amount'),
            'gt',
            Expr.number(10000)
          ),
          nextStepId: 'step_check_user_risk',
        }),
      ],
      defaultNextStepId: 'step_check_device',
    }),
    Step.decision({
      id: 'step_check_user_risk',
      name: 'Check User Risk Score',
      description: 'Evaluate user historical risk',
      branches: [
        Step.branch({
          id: 'branch_high_risk',
          label: 'High Risk Score (>70)',
          condition: Condition.simple(
            Expr.variable('$.user.riskScore'),
            'gt',
            Expr.number(70)
          ),
          nextStepId: 'step_block',
        }),
        Step.branch({
          id: 'branch_medium_risk',
          label: 'Medium Risk (>40)',
          condition: Condition.simple(
            Expr.variable('$.user.riskScore'),
            'gt',
            Expr.number(40)
          ),
          nextStepId: 'step_require_2fa',
        }),
      ],
      defaultNextStepId: 'step_check_device',
    }),
    Step.decision({
      id: 'step_check_device',
      name: 'Check Device',
      description: 'Device verification',
      branches: [
        Step.branch({
          id: 'branch_new_device',
          label: 'New Device',
          condition: Condition.simple(
            Expr.variable('$.device.isNewDevice'),
            'eq',
            Expr.boolean(true)
          ),
          nextStepId: 'step_require_2fa',
        }),
      ],
      defaultNextStepId: 'step_pass',
    }),
    Step.action({
      id: 'step_require_2fa',
      name: 'Require 2FA Verification',
      description: 'Set up 2FA requirement',
      assignments: [
        { name: 'requires2FA', value: Expr.boolean(true) },
        { name: 'riskLevel', value: Expr.string('medium') },
      ],
      logging: {
        message: Expr.string('2FA verification required'),
        level: 'warn',
      },
      nextStepId: 'step_pending_2fa',
    }),
    Step.terminal({
      id: 'step_pass',
      name: 'Low Risk - Pass',
      code: 'PASS',
      message: Expr.string('Transaction approved'),
      output: [
        { name: 'riskLevel', value: Expr.string('low') },
        { name: 'action', value: Expr.string('allow') },
      ],
    }),
    Step.terminal({
      id: 'step_pending_2fa',
      name: 'Pending 2FA',
      code: 'PENDING_2FA',
      message: Expr.string('Requires 2FA verification'),
      output: [
        { name: 'riskLevel', value: Expr.string('medium') },
        { name: 'action', value: Expr.string('challenge') },
        { name: 'challengeType', value: Expr.string('2fa') },
      ],
    }),
    Step.terminal({
      id: 'step_block',
      name: 'Block Transaction',
      code: 'BLOCKED',
      message: Expr.string('Transaction blocked due to high risk'),
      output: [
        { name: 'riskLevel', value: Expr.string('high') },
        { name: 'action', value: Expr.string('block') },
      ],
    }),
  ],
  groups: [
    {
      id: 'risk_stage_1',
      name: 'Stage 1: Initial Risk Check',
      description: 'Check transaction amount and user risk score',
      color: '#4d1e1e',
      position: { x: 0, y: 0 },
      size: { width: 400, height: 300 },
      stepIds: ['step_check_amount_risk', 'step_check_user_risk'],
    },
    {
      id: 'risk_stage_2',
      name: 'Stage 2: Device Verification',
      description: 'Verify device and apply 2FA if needed',
      color: '#3d1e5f',
      position: { x: 0, y: 0 },
      size: { width: 400, height: 300 },
      stepIds: ['step_check_device', 'step_require_2fa'],
    },
    {
      id: 'risk_stage_3',
      name: 'Stage 3: Final Decision',
      description: 'Output risk assessment result',
      color: '#1e4d2b',
      position: { x: 0, y: 0 },
      size: { width: 400, height: 300 },
      stepIds: ['step_pass', 'step_pending_2fa', 'step_block'],
    },
  ],
  metadata: {
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  },
};

const discountRuleset: RuleSet = {
  config: {
    name: 'Discount Calculator',
    version: '1.0.0',
    description: 'Calculate applicable discounts based on customer and cart',
    inputSchema: discountSchema,
    enableTrace: true,
  },
  startStepId: 'step_check_new_customer',
  steps: [
    Step.decision({
      id: 'step_check_new_customer',
      name: 'Check New Customer',
      description: 'Special handling for first-time buyers',
      branches: [
        Step.branch({
          id: 'branch_new',
          label: 'New Customer',
          condition: Condition.simple(
            Expr.variable('$.customer.isNewCustomer'),
            'eq',
            Expr.boolean(true)
          ),
          nextStepId: 'step_new_customer_bonus',
        }),
      ],
      defaultNextStepId: 'step_check_tier',
    }),
    Step.action({
      id: 'step_new_customer_bonus',
      name: 'New Customer Bonus',
      description: 'Apply new customer welcome discount',
      assignments: [
        { name: 'welcomeDiscount', value: Expr.number(0.15) },
        { name: 'bonusPoints', value: Expr.number(500) },
      ],
      logging: {
        message: Expr.string('New customer welcome offer applied'),
        level: 'info',
      },
      nextStepId: 'step_check_cart_size',
    }),
    Step.decision({
      id: 'step_check_tier',
      name: 'Check Membership Tier',
      description: 'Apply tier-based discounts',
      branches: [
        Step.branch({
          id: 'branch_platinum',
          label: 'Platinum Tier',
          condition: Condition.simple(
            Expr.variable('$.customer.tier'),
            'eq',
            Expr.string('platinum')
          ),
          nextStepId: 'step_platinum_discount',
        }),
        Step.branch({
          id: 'branch_gold_tier',
          label: 'Gold Tier',
          condition: Condition.simple(
            Expr.variable('$.customer.tier'),
            'eq',
            Expr.string('gold')
          ),
          nextStepId: 'step_gold_tier_discount',
        }),
        Step.branch({
          id: 'branch_silver_tier',
          label: 'Silver Tier',
          condition: Condition.simple(
            Expr.variable('$.customer.tier'),
            'eq',
            Expr.string('silver')
          ),
          nextStepId: 'step_silver_discount',
        }),
      ],
      defaultNextStepId: 'step_check_cart_size',
    }),
    Step.action({
      id: 'step_platinum_discount',
      name: 'Platinum Discount',
      assignments: [
        { name: 'tierDiscount', value: Expr.number(0.25) },
        { name: 'freeShipping', value: Expr.boolean(true) },
      ],
      nextStepId: 'step_check_cart_size',
    }),
    Step.action({
      id: 'step_gold_tier_discount',
      name: 'Gold Tier Discount',
      assignments: [
        { name: 'tierDiscount', value: Expr.number(0.15) },
        { name: 'freeShipping', value: Expr.boolean(true) },
      ],
      nextStepId: 'step_check_cart_size',
    }),
    Step.action({
      id: 'step_silver_discount',
      name: 'Silver Discount',
      assignments: [
        { name: 'tierDiscount', value: Expr.number(0.10) },
      ],
      nextStepId: 'step_check_cart_size',
    }),
    Step.decision({
      id: 'step_check_cart_size',
      name: 'Check Cart Size',
      description: 'Volume-based discounts',
      branches: [
        Step.branch({
          id: 'branch_large_cart',
          label: 'Large Cart (>500)',
          condition: Condition.simple(
            Expr.variable('$.cart.subtotal'),
            'gt',
            Expr.number(500)
          ),
          nextStepId: 'step_bulk_discount',
        }),
      ],
      defaultNextStepId: 'step_check_holiday',
    }),
    Step.action({
      id: 'step_bulk_discount',
      name: 'Bulk Order Discount',
      assignments: [
        { name: 'bulkDiscount', value: Expr.number(0.05) },
      ],
      nextStepId: 'step_check_holiday',
    }),
    Step.decision({
      id: 'step_check_holiday',
      name: 'Check Holiday Promotion',
      branches: [
        Step.branch({
          id: 'branch_holiday',
          label: 'Holiday Period',
          condition: Condition.simple(
            Expr.variable('$.promotion.isHoliday'),
            'eq',
            Expr.boolean(true)
          ),
          nextStepId: 'step_holiday_bonus',
        }),
      ],
      defaultNextStepId: 'step_calculate_final',
    }),
    Step.action({
      id: 'step_holiday_bonus',
      name: 'Holiday Bonus',
      assignments: [
        { name: 'holidayBonus', value: Expr.number(0.05) },
        { name: 'giftWrapping', value: Expr.boolean(true) },
      ],
      nextStepId: 'step_calculate_final',
    }),
    Step.terminal({
      id: 'step_calculate_final',
      name: 'Calculate Final Discount',
      code: 'DISCOUNT_CALCULATED',
      message: Expr.string('Discount calculation complete'),
      output: [
        { name: 'success', value: Expr.boolean(true) },
        { name: 'discountApplied', value: Expr.boolean(true) },
      ],
    }),
  ],
  groups: [
    {
      id: 'discount_stage_1',
      name: 'Stage 1: Customer Analysis',
      description: 'Analyze customer type and membership tier',
      color: '#1e3a5f',
      position: { x: 0, y: 0 },
      size: { width: 400, height: 300 },
      stepIds: ['step_check_new_customer', 'step_new_customer_bonus', 'step_check_tier', 'step_platinum_discount', 'step_gold_tier_discount', 'step_silver_discount'],
    },
    {
      id: 'discount_stage_2',
      name: 'Stage 2: Order Analysis',
      description: 'Apply volume and promotional discounts',
      color: '#4d3319',
      position: { x: 0, y: 0 },
      size: { width: 400, height: 300 },
      stepIds: ['step_check_cart_size', 'step_bulk_discount', 'step_check_holiday', 'step_holiday_bonus'],
    },
    {
      id: 'discount_stage_3',
      name: 'Stage 3: Final Calculation',
      description: 'Calculate and output final discount',
      color: '#1e4d2b',
      position: { x: 0, y: 0 },
      size: { width: 400, height: 300 },
      stepIds: ['step_calculate_final'],
    },
  ],
  metadata: {
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  },
};

// Initialize files
const files = ref<OrdoFile[]>([
  { id: 'file_1', name: 'payment_validation.ordo', ruleset: JSON.parse(JSON.stringify(paymentRuleset)), modified: false },
  { id: 'file_2', name: 'risk_assessment.ordo', ruleset: JSON.parse(JSON.stringify(riskRuleset)), modified: false },
  { id: 'file_3', name: 'discount_calculator.ordo', ruleset: JSON.parse(JSON.stringify(discountRuleset)), modified: false },
]);

const activeFileId = ref<string>('file_1');
const openTabs = ref<string[]>(['file_1']);

const activeFile = computed(() => files.value.find(f => f.id === activeFileId.value));
const ruleset = computed({
  get: () => activeFile.value?.ruleset ?? paymentRuleset,
  set: (newVal) => {
    const file = files.value.find(f => f.id === activeFileId.value);
    if (file) {
      file.ruleset = newVal;
      file.modified = true;
    }
  }
});

// Current file schema
const currentSchema = computed(() => {
  return activeFile.value?.ruleset.config.inputSchema || [];
});

// Field suggestions for expressions
const suggestions = computed(() => {
  const result: { path: string; label: string; type: string; description?: string }[] = [];
  
  function flatten(fields: SchemaField[], prefix = '') {
    for (const field of fields) {
      const path = prefix ? `${prefix}.${field.name}` : field.name;
      result.push({
        path: `$.${path}`,
        label: field.name,
        type: field.type,
        description: field.description,
      });
      if (field.type === 'object' && field.fields) {
        flatten(field.fields, path);
      }
    }
  }
  
  flatten(currentSchema.value);
  return result;
});

const jsonOutput = computed(() => {
  try {
    return serializeRuleSet(ruleset.value, { pretty: true });
  } catch {
    return '{}';
  }
});

// File operations
function selectFile(fileId: string) {
  activeFileId.value = fileId;
  if (!openTabs.value.includes(fileId)) {
    openTabs.value.push(fileId);
  }
}

function closeTab(fileId: string) {
  const idx = openTabs.value.indexOf(fileId);
  if (idx !== -1) {
    openTabs.value.splice(idx, 1);
    if (activeFileId.value === fileId) {
      activeFileId.value = openTabs.value[Math.max(0, idx - 1)] || '';
    }
  }
}

function createNewFile() {
  const id = `file_${generateId()}`;
  const newFile: OrdoFile = {
    id,
    name: `new_rule_${files.value.length + 1}.ordo`,
    ruleset: {
      config: {
        name: 'New Rule',
        version: '1.0.0',
        description: '',
        inputSchema: [],
        enableTrace: true,
      },
      startStepId: '',
      steps: [],
      metadata: {
        createdAt: new Date().toISOString(),
        updatedAt: new Date().toISOString(),
      },
    },
    modified: true,
  };
  files.value.push(newFile);
  selectFile(id);
}

function deleteFile(fileId: string) {
  const idx = files.value.findIndex(f => f.id === fileId);
  if (idx !== -1) {
    files.value.splice(idx, 1);
    closeTab(fileId);
    if (files.value.length > 0 && !activeFileId.value) {
      selectFile(files.value[0].id);
    }
  }
}

function getFileIcon(file: OrdoFile) {
  const stepTypes = file.ruleset.steps.map(s => s.type);
  if (stepTypes.includes('decision') && stepTypes.includes('terminal')) {
    return 'decision';
  } else if (stepTypes.every(s => s === 'action')) {
    return 'action';
  }
  return 'terminal';
}

function getStepTypeCounts(file: OrdoFile) {
  const counts = { decision: 0, action: 0, terminal: 0 };
  for (const step of file.ruleset.steps) {
    if (step.type in counts) {
      counts[step.type as keyof typeof counts]++;
    }
  }
  return counts;
}

async function copyJson() {
  try {
    await navigator.clipboard.writeText(jsonOutput.value);
  } catch {
    console.error('Failed to copy');
  }
}

function handleChange(newRuleset: RuleSet) {
  ruleset.value = newRuleset;
}

watch(theme, (newTheme) => {
  document.documentElement.setAttribute('data-ordo-theme', newTheme);
}, { immediate: true });
</script>

<template>
  <div class="ide-layout" :class="[theme, { 'resizing': isResizingLeft || isResizingRight }]">
    <!-- Activity Bar (Far Left) -->
    <aside class="ide-activity-bar">
      <!-- Explorer toggle -->
      <div 
        class="activity-icon" 
        :class="{ active: showLeftSidebar }"
        @click="toggleLeftSidebar"
        title="Toggle Explorer"
      >
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M3 3h7v7H3zM14 3h7v4h-7zM14 10h7v11h-7zM3 13h7v8H3z"/>
        </svg>
      </div>
      
      <!-- Form Mode -->
      <div 
        class="activity-icon" 
        :class="{ active: editorMode === 'form' }"
        @click="setEditorMode('form')"
        title="Form Editor"
      >
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <rect x="3" y="3" width="18" height="18" rx="2"/>
          <line x1="7" y1="8" x2="17" y2="8"/>
          <line x1="7" y1="12" x2="17" y2="12"/>
          <line x1="7" y1="16" x2="13" y2="16"/>
        </svg>
      </div>
      
      <!-- Flow Mode -->
      <div 
        class="activity-icon" 
        :class="{ active: editorMode === 'flow' }"
        @click="setEditorMode('flow')"
        title="Flow Editor"
      >
        <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="12" cy="5" r="3"/>
          <circle cx="6" cy="19" r="3"/>
          <circle cx="18" cy="19" r="3"/>
          <line x1="12" y1="8" x2="12" y2="12"/>
          <line x1="12" y1="12" x2="6" y2="16"/>
          <line x1="12" y1="12" x2="18" y2="16"/>
        </svg>
      </div>
      
      <div class="spacer"></div>
      
      <!-- JSON Panel toggle -->
      <div 
        class="activity-icon" 
        :class="{ active: showRightSidebar }"
        @click="toggleRightSidebar"
        title="Toggle JSON Output"
      >
        <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M8 3H5a2 2 0 0 0-2 2v3m18 0V5a2 2 0 0 0-2-2h-3m0 18h3a2 2 0 0 0 2-2v-3M3 16v3a2 2 0 0 0 2 2h3"/>
          <path d="M7 13l3 3-3 3"/>
          <path d="M17 11l-3-3 3-3"/>
        </svg>
      </div>
      
      <div class="activity-icon" @click="toggleLocale" :title="locale === 'en' ? 'Switch to Chinese' : 'Switch to English'">
        <span style="font-size: 10px; font-weight: 700;">{{ locale === 'en' ? 'EN' : '中' }}</span>
      </div>
      <div class="activity-icon" @click="toggleTheme" :title="theme === 'light' ? 'Switch to Dark Mode' : 'Switch to Light Mode'">
        <svg v-if="theme === 'light'" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"></path></svg>
        <svg v-else width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="5"></circle><line x1="12" y1="1" x2="12" y2="3"></line><line x1="12" y1="21" x2="12" y2="23"></line><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"></line><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"></line><line x1="1" y1="12" x2="3" y2="12"></line><line x1="21" y1="12" x2="23" y2="12"></line><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"></line><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"></line></svg>
      </div>
    </aside>

    <!-- Left Sidebar (Explorer) -->
    <aside 
      v-if="showLeftSidebar" 
      class="ide-sidebar left"
      :style="{ width: leftSidebarWidth + 'px' }"
    >
      <div class="sidebar-header">
        <span>EXPLORER</span>
        <div class="header-actions">
          <button class="icon-btn" @click="createNewFile" title="New File">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="12" y1="5" x2="12" y2="19"></line>
              <line x1="5" y1="12" x2="19" y2="12"></line>
            </svg>
          </button>
          <button class="sidebar-close" @click="toggleLeftSidebar" title="Close">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>
      </div>
      <div class="sidebar-content">
        <div class="sidebar-section">
          <div class="section-title">RULE FILES</div>
          <div 
            v-for="file in files" 
            :key="file.id"
            class="file-item"
            :class="{ active: file.id === activeFileId }"
            @click="selectFile(file.id)"
          >
            <div class="file-main">
              <OrdoIcon :name="getFileIcon(file)" :size="14" class="file-icon" />
              <span class="file-name">{{ file.name }}</span>
              <span v-if="file.modified" class="modified-dot">●</span>
            </div>
            <div class="file-meta">
              <span class="step-count" :class="{ 'has-decision': getStepTypeCounts(file).decision > 0 }">
                <span v-if="getStepTypeCounts(file).decision > 0" class="step-badge decision">
                  {{ getStepTypeCounts(file).decision }}D
                </span>
                <span v-if="getStepTypeCounts(file).action > 0" class="step-badge action">
                  {{ getStepTypeCounts(file).action }}A
                </span>
                <span v-if="getStepTypeCounts(file).terminal > 0" class="step-badge terminal">
                  {{ getStepTypeCounts(file).terminal }}T
                </span>
              </span>
            </div>
            <button 
              v-if="files.length > 1"
              class="file-delete" 
              @click.stop="deleteFile(file.id)"
              title="Delete"
            >
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="18" y1="6" x2="6" y2="18"></line>
                <line x1="6" y1="6" x2="18" y2="18"></line>
              </svg>
            </button>
          </div>
        </div>
        
        <!-- File Info -->
        <div v-if="activeFile" class="sidebar-section">
          <div class="section-title">FILE INFO</div>
          <div class="file-info">
            <div class="info-row">
              <span class="info-label">Name:</span>
              <span class="info-value">{{ activeFile.ruleset.config.name }}</span>
            </div>
            <div class="info-row">
              <span class="info-label">Version:</span>
              <span class="info-value">{{ activeFile.ruleset.config.version }}</span>
            </div>
            <div class="info-row">
              <span class="info-label">Steps:</span>
              <span class="info-value">{{ activeFile.ruleset.steps.length }}</span>
            </div>
          </div>
        </div>
        
        <!-- Step Types Legend -->
        <div class="sidebar-section">
          <div class="section-title">STEP TYPES</div>
          <div class="legend">
            <div class="legend-item">
              <span class="legend-color decision"></span>
              <span>Decision - Branch logic</span>
            </div>
            <div class="legend-item">
              <span class="legend-color action"></span>
              <span>Action - Set variables, logging</span>
            </div>
            <div class="legend-item">
              <span class="legend-color terminal"></span>
              <span>Terminal - End with output</span>
            </div>
          </div>
        </div>
      </div>
      <!-- Resize handle -->
      <div class="resize-handle right" @mousedown="startResizeLeft"></div>
    </aside>

    <!-- Main Editor Area -->
    <main class="ide-editor-area">
      <!-- Tabs -->
      <div class="ide-tabs">
        <div 
          v-for="tabId in openTabs" 
          :key="tabId"
          class="ide-tab"
          :class="{ active: tabId === activeFileId }"
          @click="selectFile(tabId)"
        >
          <OrdoIcon :name="getFileIcon(files.find(f => f.id === tabId)!)" :size="14" />
          {{ files.find(f => f.id === tabId)?.name }}
          <span v-if="files.find(f => f.id === tabId)?.modified" class="modified-indicator">●</span>
          <span class="mode-badge">{{ editorMode === 'form' ? 'Form' : 'Flow' }}</span>
          <span class="close" @click.stop="closeTab(tabId)">×</span>
        </div>
      </div>

      <!-- Editor Content -->
      <div class="ide-editor-wrapper" v-if="activeFile">
        <div class="ide-editor-content">
        <!-- Form Editor -->
        <OrdoFormEditor
          v-if="editorMode === 'form'"
          v-model="ruleset"
          :auto-validate="true"
          :show-validation="true"
          :locale="locale"
          @change="handleChange"
        />
        
        <!-- Flow Editor -->
        <OrdoFlowEditor
          v-else
          v-model="ruleset"
          :suggestions="suggestions"
            :locale="locale"
            :execution-trace="executionTrace"
          @change="handleChange"
          />
        </div>
        
        <!-- Execution Panel (Bottom) -->
        <OrdoExecutionPanel
          v-model:visible="showExecutionPanel"
          v-model:height="executionPanelHeight"
          :ruleset="ruleset"
          :sample-input="currentSampleInput"
          @show-in-flow="onShowInFlow"
          @clear-flow-trace="onClearFlowTrace"
        />
      </div>
      
      <!-- Empty state -->
      <div v-else class="empty-state">
        <OrdoIcon name="terminal" :size="48" />
        <p>No file selected</p>
        <button class="create-btn" @click="createNewFile">Create New Rule</button>
      </div>
      
      <!-- Status Bar -->
      <footer class="ide-status-bar">
        <div class="status-item">Ordo v0.1.0</div>
        <div class="status-item">
          <OrdoIcon name="check" :size="12" /> Ready
        </div>
        <div class="spacer"></div>
        <div v-if="activeFile" class="status-item clickable" :class="{ active: showExecutionPanel }" @click="toggleExecutionPanel" title="Toggle Execution Panel">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor" style="margin-right: 4px;">
            <path d="M8 5v14l11-7z"/>
          </svg>
          {{ showExecutionPanel ? 'Hide Console' : 'Console' }}
        </div>
        <div v-if="activeFile" class="status-item">
          {{ activeFile.ruleset.steps.length }} steps
        </div>
        <div class="status-item clickable" @click="setEditorMode(editorMode === 'form' ? 'flow' : 'form')">
          Mode: {{ editorMode === 'form' ? 'Form' : 'Flow' }}
        </div>
        <div class="status-item clickable" @click="toggleRightSidebar">
          JSON: {{ showRightSidebar ? 'On' : 'Off' }}
        </div>
        <div class="status-item">Spaces: 2</div>
        <div class="status-item clickable" @click="toggleLocale">
          {{ locale === 'en' ? 'English' : '简体中文' }}
        </div>
      </footer>
    </main>

    <!-- Right Sidebar (JSON Output) -->
    <aside 
      v-if="showRightSidebar" 
      class="ide-sidebar right"
      :style="{ width: rightSidebarWidth + 'px' }"
    >
      <!-- Resize handle -->
      <div class="resize-handle left" @mousedown="startResizeRight"></div>
      
      <div class="sidebar-header">
        <span>JSON OUTPUT</span>
        <div class="header-actions">
          <button class="icon-btn" @click="copyJson" title="Copy JSON">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <rect x="9" y="9" width="13" height="13" rx="2" ry="2"></rect>
              <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"></path>
            </svg>
          </button>
          <button class="sidebar-close" @click="toggleRightSidebar" title="Close">
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="18" y1="6" x2="6" y2="18"></line>
              <line x1="6" y1="6" x2="18" y2="18"></line>
            </svg>
          </button>
        </div>
      </div>
      <div class="sidebar-content json-panel">
        <pre class="json-code"><code>{{ jsonOutput }}</code></pre>
      </div>
    </aside>

  </div>
</template>

<style scoped>
.ide-layout {
  display: flex;
  height: 100vh;
  width: 100vw;
  background: var(--ordo-bg-app);
  color: var(--ordo-text-primary);
  font-family: var(--ordo-font-sans);
  overflow: hidden;
}

.ide-layout.resizing {
  cursor: col-resize;
  user-select: none;
}

/* Activity Bar */
.ide-activity-bar {
  width: 48px;
  background: var(--ordo-bg-secondary, #1a1a1a);
  border-right: 1px solid var(--ordo-border-color);
  display: flex;
  flex-direction: column;
  align-items: center;
  padding-top: 8px;
  z-index: 10;
  flex-shrink: 0;
}

.activity-icon {
  width: 40px;
  height: 40px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  opacity: 0.5;
  transition: opacity 0.2s, background 0.2s;
  color: var(--ordo-text-secondary);
  border-radius: 6px;
  margin: 2px 4px;
}

.activity-icon:hover {
  opacity: 0.8;
  background: var(--ordo-bg-item-hover);
}

.activity-icon.active {
  opacity: 1;
  color: var(--ordo-text-primary);
  background: var(--ordo-bg-selected);
}

.spacer { flex: 1; }

/* Sidebars */
.ide-sidebar {
  background: var(--ordo-bg-panel);
  border-color: var(--ordo-border-color);
  display: flex;
  flex-direction: column;
  position: relative;
  flex-shrink: 0;
}

.ide-sidebar.left {
  border-right: 1px solid var(--ordo-border-color);
}

.ide-sidebar.right {
  border-left: 1px solid var(--ordo-border-color);
}

.sidebar-header {
  padding: 8px 12px;
  font-size: 11px;
  font-weight: 600;
  color: var(--ordo-text-tertiary);
  letter-spacing: 0.5px;
  border-bottom: 1px solid var(--ordo-border-color);
  display: flex;
  justify-content: space-between;
  align-items: center;
  flex-shrink: 0;
}

.header-actions {
  display: flex;
  gap: 4px;
}

.sidebar-close, .icon-btn {
  background: transparent;
  border: none;
  cursor: pointer;
  color: var(--ordo-text-tertiary);
  padding: 2px;
  border-radius: 3px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.sidebar-close:hover, .icon-btn:hover {
  background: var(--ordo-bg-item-hover);
  color: var(--ordo-text-primary);
}

.sidebar-content {
  flex: 1;
  overflow-y: auto;
}

.sidebar-section {
  padding: 8px 0;
  border-bottom: 1px solid var(--ordo-border-light);
}

.sidebar-section:last-child {
  border-bottom: none;
}

.section-title {
  padding: 4px 12px;
  font-size: 10px;
  font-weight: 600;
  color: var(--ordo-text-tertiary);
  letter-spacing: 0.5px;
}

/* File items */
.file-item {
  padding: 6px 12px;
  font-size: 12px;
  color: var(--ordo-text-secondary);
  cursor: pointer;
  display: flex;
  flex-direction: column;
  gap: 4px;
  position: relative;
  border-left: 2px solid transparent;
  transition: all 0.15s;
}

.file-item:hover {
  background: var(--ordo-bg-item-hover);
  color: var(--ordo-text-primary);
}

.file-item.active {
  background: var(--ordo-bg-selected);
  color: var(--ordo-text-primary);
  border-left-color: var(--ordo-accent);
}

.file-main {
  display: flex;
  align-items: center;
  gap: 6px;
}

.file-icon {
  opacity: 0.7;
  flex-shrink: 0;
}

.file-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.modified-dot {
  color: var(--ordo-accent);
  font-size: 8px;
}

.file-meta {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-left: 20px;
}

.step-count {
  display: flex;
  gap: 4px;
}

.step-badge {
  font-size: 9px;
  font-weight: 600;
  padding: 1px 4px;
  border-radius: 2px;
  font-family: var(--ordo-font-mono);
}

.step-badge.decision {
  background: rgba(183, 110, 0, 0.2);
  color: #e8a835;
}

.step-badge.action {
  background: rgba(0, 122, 204, 0.2);
  color: #3794ff;
}

.step-badge.terminal {
  background: rgba(40, 167, 69, 0.2);
  color: #4ec969;
}

.file-delete {
  position: absolute;
  right: 8px;
  top: 50%;
  transform: translateY(-50%);
  background: transparent;
  border: none;
  cursor: pointer;
  color: var(--ordo-text-tertiary);
  padding: 2px;
  border-radius: 3px;
  opacity: 0;
  transition: opacity 0.15s;
}

.file-item:hover .file-delete {
  opacity: 1;
}

.file-delete:hover {
  background: var(--ordo-danger);
  color: #fff;
}

/* File info */
.file-info {
  padding: 4px 12px;
}

.info-row {
  display: flex;
  justify-content: space-between;
  padding: 2px 0;
  font-size: 11px;
}

.info-label {
  color: var(--ordo-text-tertiary);
}

.info-value {
  color: var(--ordo-text-secondary);
}

/* Legend */
.legend {
  padding: 4px 12px;
}

.legend-item {
  display: flex;
  align-items: center;
  gap: 8px;
  font-size: 11px;
  color: var(--ordo-text-secondary);
  padding: 3px 0;
}

.legend-color {
  width: 12px;
  height: 12px;
  border-radius: 2px;
}

.legend-color.decision {
  background: var(--ordo-node-decision, #b76e00);
}

.legend-color.action {
  background: var(--ordo-node-action, #007acc);
}

.legend-color.terminal {
  background: var(--ordo-node-terminal, #28a745);
}

/* Resize handles */
.resize-handle {
  position: absolute;
  top: 0;
  bottom: 0;
  width: 4px;
  cursor: col-resize;
  z-index: 100;
  transition: background 0.15s;
}

.resize-handle:hover,
.resize-handle:active {
  background: var(--ordo-accent);
}

.resize-handle.right {
  right: -2px;
}

.resize-handle.left {
  left: -2px;
}

/* JSON Panel */
.json-panel {
  padding: 0;
}

.json-code {
  margin: 0;
  padding: 12px;
  font-family: var(--ordo-font-mono);
  font-size: 11px;
  line-height: 1.5;
  color: var(--ordo-text-primary);
  overflow: auto;
  height: 100%;
  white-space: pre;
  word-break: break-all;
}

/* Editor Area */
.ide-editor-area {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  background: var(--ordo-bg-editor);
  overflow: hidden;
}

.ide-tabs {
  height: 35px;
  background: var(--ordo-bg-panel);
  display: flex;
  border-bottom: 1px solid var(--ordo-border-color);
  overflow-x: auto;
  flex-shrink: 0;
}

.ide-tab {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 16px;
  font-size: 13px;
  color: var(--ordo-text-secondary);
  border-right: 1px solid var(--ordo-border-color);
  background: var(--ordo-bg-panel);
  cursor: pointer;
  min-width: 120px;
  position: relative;
}

.ide-tab.active {
  background: var(--ordo-bg-editor);
  color: var(--ordo-text-primary);
  border-top: 2px solid var(--ordo-accent);
}

.ide-tab .modified-indicator {
  color: var(--ordo-accent);
  font-size: 10px;
  position: absolute;
  top: 4px;
  left: 8px;
}

.ide-tab .mode-badge {
  font-size: 9px;
  font-weight: 600;
  color: var(--ordo-accent);
  background: var(--ordo-accent-bg);
  padding: 2px 6px;
  border-radius: 3px;
  text-transform: uppercase;
}

.ide-tab .close {
  margin-left: auto;
  opacity: 0;
  font-size: 14px;
}

.ide-tab:hover .close {
  opacity: 1;
}

.ide-editor-wrapper {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  min-height: 0;
}

.ide-editor-content {
  flex: 1;
  overflow: hidden;
  position: relative;
  min-height: 0;
}

/* Empty state */
.empty-state {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  color: var(--ordo-text-tertiary);
  gap: 16px;
}

.empty-state p {
  margin: 0;
  font-size: 14px;
}

.create-btn {
  background: var(--ordo-accent);
  color: #fff;
  border: none;
  padding: 8px 16px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
}

.create-btn:hover {
  opacity: 0.9;
}

/* Status Bar */
.ide-status-bar {
  height: 22px;
  background: var(--ordo-accent);
  color: #fff;
  display: flex;
  align-items: center;
  padding: 0 8px;
  font-size: 11px;
  flex-shrink: 0;
}

.status-item {
  padding: 0 8px;
  display: flex;
  align-items: center;
  gap: 4px;
}

.status-item.clickable {
  cursor: pointer;
}

.status-item.clickable:hover {
  background: rgba(255, 255, 255, 0.1);
}

.status-item.clickable.active {
  background: rgba(255, 255, 255, 0.2);
}

.status-item:hover {
  background: rgba(255,255,255,0.2);
}
</style>
