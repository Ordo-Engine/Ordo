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
