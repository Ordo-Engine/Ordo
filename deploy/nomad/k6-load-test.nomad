# Ordo Rule Engine - k6 Load Test Job (Batch)
#
# This job runs k6 load tests against the Ordo server.
# It is a batch job that will run once and complete.
#
# Usage examples:
#   # Run with defaults (health endpoint, 10 VUs, 30s duration)
#   nomad job run k6-load-test.nomad
#
#   # Run with custom parameters via dispatch
#   nomad job dispatch \
#     -meta target_endpoint=execute \
#     -meta vus=50 \
#     -meta duration=60s \
#     k6-load-test
#
# Available target_endpoint values:
#   - health      : GET /health
#   - list        : GET /api/v1/rulesets
#   - execute     : POST /api/v1/execute/:name (requires ruleset_name)
#   - eval        : POST /api/v1/eval
#   - get_ruleset : GET /api/v1/rulesets/:name (requires ruleset_name)

job "k6-load-test" {
  datacenters = ["dc1"]
  type        = "batch"

  # Parameterized job allows for different test configurations
  parameterized {
    meta_required = []
    meta_optional = [
      "target_endpoint",
      "target_url",
      "vus",
      "duration",
      "rps",
      "ruleset_name",
      "expression",
      "input_json",
      "context_json",
    ]
  }

  # Default meta values
  meta {
    target_endpoint = "health"
    # Note: You need to update this with the Ordo container's IP address
    # Run: docker inspect <ordo-container-name> --format '{{.NetworkSettings.Networks.bridge.IPAddress}}'
    # Current Ordo container IP (may change after restart)
    target_url      = "http://172.17.0.3:8080"
    vus             = "10"
    duration        = "30s"
    rps             = "0"
    ruleset_name    = "demo"
    expression      = "1 + 1"
    input_json      = "{\"amount\": 1000, \"score\": 75}"
    context_json    = "{\"x\": 10, \"y\": 20}"
  }

  group "k6" {
    count = 1

    # Ephemeral disk for test results
    ephemeral_disk {
      size = 200  # MB
    }

    task "load-test" {
      driver = "docker"

      config {
        image   = "grafana/k6:latest"
        command = "run"
        args = [
          "--out", "json=/alloc/data/results.json",
          "/local/script.js",
        ]
        # Use extra_hosts to enable host.docker.internal on Linux
        extra_hosts = ["host.docker.internal:host-gateway"]
      }

      # k6 test script template
      template {
        destination = "local/script.js"
        data        = <<-EOF
import http from 'k6/http';
import { check, sleep } from 'k6';
import { Rate, Trend } from 'k6/metrics';

// Custom metrics
const errorRate = new Rate('errors');
const requestDuration = new Trend('request_duration');

// Configuration from Nomad meta
const CONFIG = {
  targetEndpoint: '{{ env "NOMAD_META_target_endpoint" }}',
  targetUrl: '{{ env "NOMAD_META_target_url" }}',
  rulesetName: '{{ env "NOMAD_META_ruleset_name" }}',
  expression: '{{ env "NOMAD_META_expression" }}',
  inputJson: '{{ env "NOMAD_META_input_json" }}',
  contextJson: '{{ env "NOMAD_META_context_json" }}',
};

// k6 options
export const options = {
  vus: parseInt('{{ env "NOMAD_META_vus" }}') || 10,
  duration: '{{ env "NOMAD_META_duration" }}' || '30s',
  thresholds: {
    http_req_duration: [
      'p(95)<500',
      'p(99)<1000',
    ],
    errors: ['rate<0.01'],
  },
  tags: {
    testType: 'load',
    endpoint: CONFIG.targetEndpoint,
  },
};

// Endpoint handlers
const endpoints = {
  // Health check endpoint
  health: () => {
    return http.get(`${CONFIG.targetUrl}/health`);
  },

  // List rulesets endpoint
  list: () => {
    return http.get(`${CONFIG.targetUrl}/api/v1/rulesets`);
  },

  // Get specific ruleset endpoint
  get_ruleset: () => {
    return http.get(`${CONFIG.targetUrl}/api/v1/rulesets/${CONFIG.rulesetName}`);
  },

  // Execute ruleset endpoint
  execute: () => {
    const payload = JSON.stringify({
      input: JSON.parse(CONFIG.inputJson),
      trace: false,
    });
    const params = {
      headers: { 'Content-Type': 'application/json' },
    };
    return http.post(
      `${CONFIG.targetUrl}/api/v1/execute/${CONFIG.rulesetName}`,
      payload,
      params
    );
  },

  // Eval expression endpoint
  eval: () => {
    const payload = JSON.stringify({
      expression: CONFIG.expression,
      context: JSON.parse(CONFIG.contextJson),
    });
    const params = {
      headers: { 'Content-Type': 'application/json' },
    };
    return http.post(`${CONFIG.targetUrl}/api/v1/eval`, payload, params);
  },
};

// Setup function - runs once before test
export function setup() {
  console.log('='.repeat(60));
  console.log('Ordo k6 Load Test');
  console.log('='.repeat(60));
  console.log('Target URL:      ' + CONFIG.targetUrl);
  console.log('Endpoint:        ' + CONFIG.targetEndpoint);
  console.log('Virtual Users:   ' + options.vus);
  console.log('Duration:        ' + options.duration);
  console.log('='.repeat(60));

  // Verify target is reachable
  const healthRes = http.get(CONFIG.targetUrl + '/health');
  if (healthRes.status !== 200) {
    console.error('Health check failed: ' + healthRes.status);
    return { error: true };
  }
  console.log('Health check passed, starting load test...');
  return { error: false };
}

// Main test function
export default function(data) {
  if (data.error) {
    console.error('Skipping test due to setup error');
    return;
  }

  const endpoint = endpoints[CONFIG.targetEndpoint];
  if (!endpoint) {
    console.error('Unknown endpoint: ' + CONFIG.targetEndpoint);
    errorRate.add(1);
    return;
  }

  const startTime = Date.now();
  const res = endpoint();
  const duration = Date.now() - startTime;

  // Record custom metrics
  requestDuration.add(duration);

  // Check response
  const success = check(res, {
    'status is 2xx': (r) => r.status >= 200 && r.status < 300,
    'response has body': (r) => r.body && r.body.length > 0,
  });

  errorRate.add(!success);

  // Small sleep to prevent hammering
  sleep(0.01);
}

// Teardown function - runs once after test
export function teardown(data) {
  console.log('='.repeat(60));
  console.log('Load test completed');
  console.log('='.repeat(60));
}
EOF
      }

      # Environment variables
      env {
        K6_OUT = "json"
      }

      # Resource allocation for k6
      resources {
        cpu    = 500   # MHz
        memory = 256   # MB
      }

      # Log collection
      logs {
        max_files     = 3
        max_file_size = 10
      }
    }

    # Optional: Results collector task
    task "collect-results" {
      driver = "docker"
      
      lifecycle {
        hook    = "poststop"
        sidecar = false
      }

      config {
        image   = "alpine:latest"
        command = "/bin/sh"
        args = [
          "-c",
          "cat /alloc/data/results.json 2>/dev/null || echo 'No results found'",
        ]
      }

      resources {
        cpu    = 50
        memory = 32
      }

      # Minimal log storage for this short-lived task
      logs {
        max_files     = 1
        max_file_size = 5
      }
    }
  }
}
