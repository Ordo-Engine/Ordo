#!/bin/bash
# Ordo Server Benchmark Script
# Uses k6 for load testing
#
# Usage: ./benchmark.sh [options]
#   -h, --host      Server host (default: localhost)
#   -p, --port      Server port (default: 8080)
#   -d, --duration  Test duration (default: 30s)
#   -u, --vus       Virtual users (default: 10)
#   -r, --rps       Target requests per second (default: 100)
#   --rule          Rule name to test (default: order_discount)
#   --analyze       Show CPU efficiency analysis after test
#   --help          Show this help message

set -e

# Default values
HOST="localhost"
PORT="8080"
DURATION="30s"
VUS="10"
RPS="100"
RULE_NAME="order_discount"
ANALYZE_CPU="false"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m'

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -h|--host)
            HOST="$2"
            shift 2
            ;;
        -p|--port)
            PORT="$2"
            shift 2
            ;;
        -d|--duration)
            DURATION="$2"
            shift 2
            ;;
        -u|--vus)
            VUS="$2"
            shift 2
            ;;
        -r|--rps)
            RPS="$2"
            shift 2
            ;;
        --rule)
            RULE_NAME="$2"
            shift 2
            ;;
        --analyze)
            ANALYZE_CPU="true"
            shift
            ;;
        --help)
            echo "Ordo Server Benchmark Script"
            echo ""
            echo "Usage: $0 [options]"
            echo ""
            echo "Options:"
            echo "  -h, --host      Server host (default: localhost)"
            echo "  -p, --port      Server port (default: 8080)"
            echo "  -d, --duration  Test duration (default: 30s)"
            echo "  -u, --vus       Virtual users (default: 10)"
            echo "  -r, --rps       Target requests per second (default: 100)"
            echo "  --rule          Rule name to test (default: order_discount)"
            echo "  --analyze       Show CPU efficiency analysis after test"
            echo "  --help          Show this help message"
            echo ""
            echo "Examples:"
            echo "  $0 -h 192.168.1.100 -p 8080 -d 60s -u 50 -r 500"
            echo "  $0 --host localhost --port 8080 --duration 2m --vus 100"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

BASE_URL="http://${HOST}:${PORT}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
K6_SCRIPT="${SCRIPT_DIR}/k6_benchmark.js"

echo -e "${CYAN}╔══════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║              Ordo Server Benchmark                               ║${NC}"
echo -e "${CYAN}╚══════════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Check if k6 is installed
if ! command -v k6 &> /dev/null; then
    echo -e "${RED}Error: k6 is not installed${NC}"
    echo ""
    echo "Install k6:"
    echo "  macOS:  brew install k6"
    echo "  Linux:  sudo apt install k6  (or snap install k6)"
    echo "  Docker: docker run -i grafana/k6 run -"
    echo ""
    exit 1
fi

# Check server health
echo -e "${YELLOW}[1/4] Checking server health...${NC}"
HEALTH_BODY=$(curl -s "${BASE_URL}/health" 2>/dev/null || echo "{}")
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" "${BASE_URL}/health" 2>/dev/null || echo "000")

if [ "$HTTP_CODE" != "200" ]; then
    echo -e "${RED}Error: Server is not responding (HTTP $HTTP_CODE)${NC}"
    echo "URL: ${BASE_URL}/health"
    exit 1
fi

VERSION=$(echo "$HEALTH_BODY" | jq -r '.version // "unknown"')
RULES_COUNT=$(echo "$HEALTH_BODY" | jq -r '.storage.rules_count // 0')
echo -e "${GREEN}  ✓ Server is healthy (v${VERSION}, ${RULES_COUNT} rules)${NC}"

# Check if rule exists
echo -e "${YELLOW}[2/4] Checking rule '${RULE_NAME}'...${NC}"
RULE_CODE=$(curl -s -o /dev/null -w "%{http_code}" "${BASE_URL}/api/v1/rulesets/${RULE_NAME}" 2>/dev/null || echo "000")

if [ "$RULE_CODE" != "200" ]; then
    echo -e "${RED}Error: Rule '${RULE_NAME}' not found${NC}"
    echo "Available rules:"
    curl -s "${BASE_URL}/api/v1/rulesets" | jq -r '.[].name' 2>/dev/null | while read name; do
        echo "  - $name"
    done
    exit 1
fi
echo -e "${GREEN}  ✓ Rule '${RULE_NAME}' exists${NC}"

# Generate k6 script
echo -e "${YELLOW}[3/4] Generating k6 script...${NC}"

cat > "${K6_SCRIPT}" << 'EOFK6'
import http from 'k6/http';
import { check } from 'k6';
import { Rate, Trend, Counter } from 'k6/metrics';

// Custom metrics
const errorRate = new Rate('errors');
const requestDuration = new Trend('request_duration_ms');
const successCounter = new Counter('successful_requests');
const failedCounter = new Counter('failed_requests');

// Test configuration from environment variables
const BASE_URL = __ENV.BASE_URL || 'http://localhost:8080';
const RULE_NAME = __ENV.RULE_NAME || 'order_discount';
const TARGET_RPS = parseInt(__ENV.TARGET_RPS) || 100;

export const options = {
    scenarios: {
        constant_load: {
            executor: 'constant-arrival-rate',
            rate: TARGET_RPS,
            timeUnit: '1s',
            duration: __ENV.DURATION || '30s',
            preAllocatedVUs: parseInt(__ENV.VUS) || 10,
            maxVUs: Math.max(parseInt(__ENV.VUS) * 3, 50),
        },
    },
    thresholds: {
        http_req_duration: ['p(95)<500', 'p(99)<1000'],
        errors: ['rate<0.05'],
    },
};

// Test data generator - only generates data matching the rule being tested
function generateInput() {
    const ruleName = RULE_NAME;
    
    if (ruleName === 'order_discount') {
        const amounts = [100, 250, 500, 750, 1000, 1500, 2000, 5000];
        return { order: { amount: amounts[Math.floor(Math.random() * amounts.length)] } };
    }
    
    if (ruleName === 'user_verification') {
        return { user: { age: Math.floor(Math.random() * 70) + 10 } };
    }
    
    if (ruleName === 'risk_assessment') {
        return { transaction: { amount: Math.floor(Math.random() * 50000) } };
    }
    
    if (ruleName === 'shipping_calculator') {
        const destinations = ['standard', 'express', 'international'];
        return { 
            destination: destinations[Math.floor(Math.random() * destinations.length)], 
            weight: Math.floor(Math.random() * 30) + 1 
        };
    }
    
    // Default fallback
    return { order: { amount: Math.floor(Math.random() * 2000) } };
}

export default function () {
    const url = `${BASE_URL}/api/v1/execute/${RULE_NAME}`;
    const payload = JSON.stringify({
        input: generateInput(),
    });
    
    const params = {
        headers: {
            'Content-Type': 'application/json',
        },
        timeout: '10s',
    };

    const startTime = Date.now();
    const response = http.post(url, payload, params);
    const duration = Date.now() - startTime;
    
    requestDuration.add(duration);

    const success = check(response, {
        'status is 200': (r) => r.status === 200,
        'response has code': (r) => {
            try {
                const body = JSON.parse(r.body);
                return body.code !== undefined;
            } catch {
                return false;
            }
        },
        'response time < 500ms': (r) => r.timings.duration < 500,
    });

    if (success) {
        successCounter.add(1);
        errorRate.add(0);
    } else {
        failedCounter.add(1);
        errorRate.add(1);
    }
}
EOFK6

echo -e "${GREEN}  ✓ k6 script generated${NC}"

# Run benchmark
echo -e "${YELLOW}[4/4] Running benchmark...${NC}"
echo ""
echo -e "${BLUE}┌─────────────────────────────────────────────┐${NC}"
echo -e "${BLUE}│ Configuration                               │${NC}"
echo -e "${BLUE}├─────────────────────────────────────────────┤${NC}"
echo -e "${BLUE}│${NC} URL:        ${BASE_URL}"
echo -e "${BLUE}│${NC} Rule:       ${RULE_NAME}"
echo -e "${BLUE}│${NC} Duration:   ${DURATION}"
echo -e "${BLUE}│${NC} VUs:        ${VUS}"
echo -e "${BLUE}│${NC} Target RPS: ${RPS}"
echo -e "${BLUE}└─────────────────────────────────────────────┘${NC}"
echo ""
echo -e "${CYAN}Starting k6...${NC}"
echo ""

# Run k6
k6 run \
    -e BASE_URL="${BASE_URL}" \
    -e RULE_NAME="${RULE_NAME}" \
    -e DURATION="${DURATION}" \
    -e VUS="${VUS}" \
    -e TARGET_RPS="${RPS}" \
    --summary-trend-stats="avg,min,med,max,p(90),p(95),p(99)" \
    "${K6_SCRIPT}"

# Cleanup
rm -f "${K6_SCRIPT}"

echo ""
echo -e "${CYAN}╔══════════════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║                    Benchmark Complete!                           ║${NC}"
echo -e "${CYAN}╚══════════════════════════════════════════════════════════════════╝${NC}"

# CPU Efficiency Analysis
if [ "$ANALYZE_CPU" = "true" ]; then
    echo ""
    echo -e "${YELLOW}[5/5] Fetching server metrics...${NC}"
    
    METRICS=$(curl -s "${BASE_URL}/metrics" 2>/dev/null || echo "")
    
    if [ -n "$METRICS" ]; then
        echo ""
        echo -e "${BLUE}┌─────────────────────────────────────────────┐${NC}"
        echo -e "${BLUE}│ Server Metrics                              │${NC}"
        echo -e "${BLUE}├─────────────────────────────────────────────┤${NC}"
        
        # Extract key metrics
        TOTAL_EXEC=$(echo "$METRICS" | grep "^ordo_executions_total" | head -1 | awk '{print $2}' || echo "N/A")
        EXEC_TIME=$(echo "$METRICS" | grep "^ordo_execution_duration_seconds_sum" | awk '{print $2}' || echo "N/A")
        UPTIME=$(echo "$METRICS" | grep "^ordo_uptime_seconds" | awk '{print $2}' || echo "N/A")
        
        echo -e "${BLUE}│${NC} Total Executions:    ${TOTAL_EXEC}"
        echo -e "${BLUE}│${NC} Total Exec Time:     ${EXEC_TIME}s"
        echo -e "${BLUE}│${NC} Server Uptime:       ${UPTIME}s"
        
        # Calculate average execution time
        if [ "$TOTAL_EXEC" != "N/A" ] && [ "$EXEC_TIME" != "N/A" ] && [ "$TOTAL_EXEC" != "0" ]; then
            AVG_EXEC_US=$(echo "scale=2; $EXEC_TIME * 1000000 / $TOTAL_EXEC" | bc 2>/dev/null || echo "N/A")
            echo -e "${BLUE}│${NC} Avg Exec Time:       ${AVG_EXEC_US}μs"
        fi
        
        echo -e "${BLUE}└─────────────────────────────────────────────┘${NC}"
        
        echo ""
        echo -e "${BLUE}┌─────────────────────────────────────────────┐${NC}"
        echo -e "${BLUE}│ CPU Efficiency Analysis                     │${NC}"
        echo -e "${BLUE}├─────────────────────────────────────────────┤${NC}"
        echo -e "${BLUE}│${NC}"
        echo -e "${BLUE}│${NC} CPU 消耗来源分析:"
        echo -e "${BLUE}│${NC}"
        echo -e "${BLUE}│${NC}   1. 规则执行 (RuleExecutor)"
        echo -e "${BLUE}│${NC}      - 表达式解析和求值"
        echo -e "${BLUE}│${NC}      - 条件判断"
        echo -e "${BLUE}│${NC}      - 预估: ~30-50μs/请求"
        echo -e "${BLUE}│${NC}"
        echo -e "${BLUE}│${NC}   2. HTTP 处理 (Axum/Hyper)"
        echo -e "${BLUE}│${NC}      - 请求解析"
        echo -e "${BLUE}│${NC}      - 路由匹配"
        echo -e "${BLUE}│${NC}      - 响应序列化"
        echo -e "${BLUE}│${NC}      - 预估: ~50-100μs/请求"
        echo -e "${BLUE}│${NC}"
        echo -e "${BLUE}│${NC}   3. JSON 序列化 (serde_json)"
        echo -e "${BLUE}│${NC}      - 请求反序列化"
        echo -e "${BLUE}│${NC}      - 响应序列化"
        echo -e "${BLUE}│${NC}      - 预估: ~20-50μs/请求"
        echo -e "${BLUE}│${NC}"
        echo -e "${BLUE}│${NC}   4. 日志记录 (tracing)"
        echo -e "${BLUE}│${NC}      - RUST_LOG=debug 时开销大"
        echo -e "${BLUE}│${NC}      - 预估: ~10-100μs/请求"
        echo -e "${BLUE}│${NC}"
        echo -e "${BLUE}│${NC}   5. Prometheus 指标"
        echo -e "${BLUE}│${NC}      - 计数器更新"
        echo -e "${BLUE}│${NC}      - 直方图记录"
        echo -e "${BLUE}│${NC}      - 预估: ~5-20μs/请求"
        echo -e "${BLUE}│${NC}"
        echo -e "${BLUE}│${NC}   6. RwLock 竞争"
        echo -e "${BLUE}│${NC}      - 获取/释放读锁"
        echo -e "${BLUE}│${NC}      - 高并发时可能增加"
        echo -e "${BLUE}│${NC}      - 预估: ~1-10μs/请求"
        echo -e "${BLUE}│${NC}"
        echo -e "${BLUE}└─────────────────────────────────────────────┘${NC}"
        
        echo ""
        echo -e "${GREEN}💡 优化建议:${NC}"
        echo "   1. 设置 RUST_LOG=warn 或 info (减少日志开销)"
        echo "   2. 考虑禁用 TraceLayer (生产环境)"
        echo "   3. 使用 release 模式编译 (--release)"
        echo "   4. 预编译表达式 (如果规则固定)"
    else
        echo -e "${RED}  ✗ Failed to fetch metrics${NC}"
    fi
fi
