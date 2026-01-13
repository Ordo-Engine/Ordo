#!/bin/bash
# Ordo Server Test Script
# Usage: ./test_server.sh [server_url]

SERVER_URL="${1:-http://219.93.129.90:24626}"
TOTAL_REQUESTS=0
SUCCESS_COUNT=0
FAIL_COUNT=0

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

echo -e "${CYAN}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║           Ordo Server Test Script                            ║${NC}"
echo -e "${CYAN}║           Target: ${SERVER_URL}${NC}"
echo -e "${CYAN}╚══════════════════════════════════════════════════════════════╝${NC}"
echo ""

# Function to make request and track results
execute_rule() {
    local name=$1
    local input=$2
    local expected_code=$3
    
    TOTAL_REQUESTS=$((TOTAL_REQUESTS + 1))
    
    response=$(curl -s -X POST "${SERVER_URL}/api/v1/execute/${name}" \
        -H "Content-Type: application/json" \
        -d "{\"input\": ${input}}")
    
    code=$(echo "$response" | jq -r '.code // "ERROR"')
    duration=$(echo "$response" | jq -r '.duration_us // 0')
    
    if [ "$code" == "$expected_code" ]; then
        SUCCESS_COUNT=$((SUCCESS_COUNT + 1))
        echo -e "  ${GREEN}✓${NC} ${code} (${duration}μs)"
        return 0
    else
        FAIL_COUNT=$((FAIL_COUNT + 1))
        echo -e "  ${RED}✗${NC} Expected: ${expected_code}, Got: ${code}"
        return 1
    fi
}

# ============================================================
# 1. Health Check
# ============================================================
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${YELLOW}[1/6] Health Check${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

health=$(curl -s "${SERVER_URL}/health")
status=$(echo "$health" | jq -r '.status')
version=$(echo "$health" | jq -r '.version')
uptime=$(echo "$health" | jq -r '.uptime_seconds')
rules_count=$(echo "$health" | jq -r '.storage.rules_count')

if [ "$status" == "healthy" ]; then
    echo -e "  ${GREEN}✓${NC} Server is healthy"
    echo -e "    Version: ${version}"
    echo -e "    Uptime: ${uptime}s"
    echo -e "    Rules loaded: ${rules_count}"
else
    echo -e "  ${RED}✗${NC} Server is not healthy!"
    exit 1
fi
echo ""

# ============================================================
# 2. Create Additional Test Rules
# ============================================================
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${YELLOW}[2/6] Creating Test Rules${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

# Rule 1: User Age Verification
echo -n "  Creating user_verification rule... "
result=$(curl -s -X POST "${SERVER_URL}/api/v1/rulesets" \
  -H "Content-Type: application/json" \
  -d '{
    "config": {
      "name": "user_verification",
      "version": "1.0.0",
      "description": "Verify user age and status",
      "entry_step": "check_age"
    },
    "steps": {
      "check_age": {
        "id": "check_age",
        "name": "Check Age",
        "type": "decision",
        "branches": [
          {
            "condition": "user.age >= 21",
            "actions": [
              {
                "action": "metric",
                "name": "adult_users",
                "value": {"Literal": 1},
                "tags": [["category", "adult"]]
              }
            ],
            "next_step": "adult_access"
          },
          {
            "condition": "user.age >= 18",
            "actions": [
              {
                "action": "metric",
                "name": "young_adult_users",
                "value": {"Literal": 1},
                "tags": [["category", "young_adult"]]
              }
            ],
            "next_step": "limited_access"
          }
        ],
        "default_next": "denied"
      },
      "adult_access": {
        "id": "adult_access",
        "name": "Full Access",
        "type": "terminal",
        "result": {
          "code": "FULL_ACCESS",
          "message": "Full access granted",
          "output": [["access_level", {"Literal": "full"}]]
        }
      },
      "limited_access": {
        "id": "limited_access",
        "name": "Limited Access",
        "type": "terminal",
        "result": {
          "code": "LIMITED_ACCESS",
          "message": "Limited access granted",
          "output": [["access_level", {"Literal": "limited"}]]
        }
      },
      "denied": {
        "id": "denied",
        "name": "Access Denied",
        "type": "terminal",
        "result": {
          "code": "ACCESS_DENIED",
          "message": "Access denied - age restriction",
          "output": [["access_level", {"Literal": "none"}]]
        }
      }
    }
  }')
echo -e "${GREEN}done${NC}"

# Rule 2: Risk Assessment
echo -n "  Creating risk_assessment rule... "
result=$(curl -s -X POST "${SERVER_URL}/api/v1/rulesets" \
  -H "Content-Type: application/json" \
  -d '{
    "config": {
      "name": "risk_assessment",
      "version": "1.0.0",
      "description": "Assess transaction risk level",
      "entry_step": "check_amount"
    },
    "steps": {
      "check_amount": {
        "id": "check_amount",
        "name": "Check Transaction Amount",
        "type": "decision",
        "branches": [
          {
            "condition": "transaction.amount >= 10000",
            "actions": [
              {
                "action": "metric",
                "name": "high_risk_transactions",
                "value": {"Field": "transaction.amount"},
                "tags": [["risk", "high"]]
              },
              {
                "action": "log",
                "message": "High risk transaction detected",
                "level": "warn"
              }
            ],
            "next_step": "high_risk"
          },
          {
            "condition": "transaction.amount >= 5000",
            "actions": [
              {
                "action": "metric",
                "name": "medium_risk_transactions",
                "value": {"Field": "transaction.amount"},
                "tags": [["risk", "medium"]]
              }
            ],
            "next_step": "medium_risk"
          },
          {
            "condition": "transaction.amount >= 1000",
            "next_step": "low_risk"
          }
        ],
        "default_next": "minimal_risk"
      },
      "high_risk": {
        "id": "high_risk",
        "name": "High Risk",
        "type": "terminal",
        "result": {
          "code": "HIGH_RISK",
          "message": "Transaction requires manual review",
          "output": [
            ["risk_level", {"Literal": "high"}],
            ["action", {"Literal": "manual_review"}]
          ]
        }
      },
      "medium_risk": {
        "id": "medium_risk",
        "name": "Medium Risk",
        "type": "terminal",
        "result": {
          "code": "MEDIUM_RISK",
          "message": "Transaction requires additional verification",
          "output": [
            ["risk_level", {"Literal": "medium"}],
            ["action", {"Literal": "verify"}]
          ]
        }
      },
      "low_risk": {
        "id": "low_risk",
        "name": "Low Risk",
        "type": "terminal",
        "result": {
          "code": "LOW_RISK",
          "message": "Transaction approved",
          "output": [
            ["risk_level", {"Literal": "low"}],
            ["action", {"Literal": "approve"}]
          ]
        }
      },
      "minimal_risk": {
        "id": "minimal_risk",
        "name": "Minimal Risk",
        "type": "terminal",
        "result": {
          "code": "MINIMAL_RISK",
          "message": "Transaction auto-approved",
          "output": [
            ["risk_level", {"Literal": "minimal"}],
            ["action", {"Literal": "auto_approve"}]
          ]
        }
      }
    }
  }')
echo -e "${GREEN}done${NC}"

# Rule 3: Shipping Calculator
echo -n "  Creating shipping_calculator rule... "
result=$(curl -s -X POST "${SERVER_URL}/api/v1/rulesets" \
  -H "Content-Type: application/json" \
  -d '{
    "config": {
      "name": "shipping_calculator",
      "version": "1.0.0",
      "description": "Calculate shipping cost based on weight and destination",
      "entry_step": "check_destination"
    },
    "steps": {
      "check_destination": {
        "id": "check_destination",
        "name": "Check Destination",
        "type": "decision",
        "branches": [
          {
            "condition": "destination == \"international\"",
            "actions": [
              {
                "action": "set_variable",
                "name": "base_rate",
                "value": {"Literal": 25.0}
              },
              {
                "action": "metric",
                "name": "international_shipments",
                "value": {"Literal": 1},
                "tags": [["type", "international"]]
              }
            ],
            "next_step": "check_weight"
          },
          {
            "condition": "destination == \"express\"",
            "actions": [
              {
                "action": "set_variable",
                "name": "base_rate",
                "value": {"Literal": 15.0}
              },
              {
                "action": "metric",
                "name": "express_shipments",
                "value": {"Literal": 1},
                "tags": [["type", "express"]]
              }
            ],
            "next_step": "check_weight"
          }
        ],
        "default_next": "standard_shipping"
      },
      "standard_shipping": {
        "id": "standard_shipping",
        "name": "Standard Shipping",
        "type": "action",
        "actions": [
          {
            "action": "set_variable",
            "name": "base_rate",
            "value": {"Literal": 5.0}
          },
          {
            "action": "metric",
            "name": "standard_shipments",
            "value": {"Literal": 1},
            "tags": [["type", "standard"]]
          }
        ],
        "next_step": "check_weight"
      },
      "check_weight": {
        "id": "check_weight",
        "name": "Check Weight",
        "type": "decision",
        "branches": [
          {
            "condition": "weight > 20",
            "actions": [
              {
                "action": "set_variable",
                "name": "weight_surcharge",
                "value": {"Literal": 10.0}
              }
            ],
            "next_step": "calculate_total"
          },
          {
            "condition": "weight > 10",
            "actions": [
              {
                "action": "set_variable",
                "name": "weight_surcharge",
                "value": {"Literal": 5.0}
              }
            ],
            "next_step": "calculate_total"
          }
        ],
        "default_next": "no_surcharge"
      },
      "no_surcharge": {
        "id": "no_surcharge",
        "name": "No Surcharge",
        "type": "action",
        "actions": [
          {
            "action": "set_variable",
            "name": "weight_surcharge",
            "value": {"Literal": 0}
          }
        ],
        "next_step": "calculate_total"
      },
      "calculate_total": {
        "id": "calculate_total",
        "name": "Calculate Total",
        "type": "terminal",
        "result": {
          "code": "SHIPPING_CALCULATED",
          "message": "Shipping cost calculated",
          "output": [
            ["base_rate", {"Field": "$base_rate"}],
            ["weight_surcharge", {"Field": "$weight_surcharge"}]
          ]
        }
      }
    }
  }')
echo -e "${GREEN}done${NC}"
echo ""

# ============================================================
# 3. Test order_discount Rule
# ============================================================
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${YELLOW}[3/6] Testing order_discount Rule${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

# Various order amounts
order_amounts=(100 250 499 500 750 999 1000 1500 2000 5000 10000)

for amount in "${order_amounts[@]}"; do
    if [ $amount -ge 1000 ]; then
        expected="DISCOUNT_APPLIED"
    elif [ $amount -ge 500 ]; then
        expected="DISCOUNT_APPLIED"
    else
        expected="NO_DISCOUNT"
    fi
    
    echo -n "  Order amount: \$${amount} -> "
    execute_rule "order_discount" "{\"order\": {\"amount\": ${amount}}}" "$expected"
done
echo ""

# ============================================================
# 4. Test user_verification Rule
# ============================================================
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${YELLOW}[4/6] Testing user_verification Rule${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

# Various ages
ages=(12 15 17 18 19 20 21 25 30 65)

for age in "${ages[@]}"; do
    if [ $age -ge 21 ]; then
        expected="FULL_ACCESS"
    elif [ $age -ge 18 ]; then
        expected="LIMITED_ACCESS"
    else
        expected="ACCESS_DENIED"
    fi
    
    echo -n "  User age: ${age} -> "
    execute_rule "user_verification" "{\"user\": {\"age\": ${age}}}" "$expected"
done
echo ""

# ============================================================
# 5. Test risk_assessment Rule
# ============================================================
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${YELLOW}[5/6] Testing risk_assessment Rule${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

# Various transaction amounts
tx_amounts=(100 500 999 1000 2500 4999 5000 7500 9999 10000 25000 100000)

for amount in "${tx_amounts[@]}"; do
    if [ $amount -ge 10000 ]; then
        expected="HIGH_RISK"
    elif [ $amount -ge 5000 ]; then
        expected="MEDIUM_RISK"
    elif [ $amount -ge 1000 ]; then
        expected="LOW_RISK"
    else
        expected="MINIMAL_RISK"
    fi
    
    echo -n "  Transaction: \$${amount} -> "
    execute_rule "risk_assessment" "{\"transaction\": {\"amount\": ${amount}}}" "$expected"
done
echo ""

# ============================================================
# 6. Test shipping_calculator Rule
# ============================================================
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${YELLOW}[6/6] Testing shipping_calculator Rule${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

# Various shipping scenarios
shipping_tests=(
    '{"destination": "standard", "weight": 5}'
    '{"destination": "standard", "weight": 15}'
    '{"destination": "standard", "weight": 25}'
    '{"destination": "express", "weight": 5}'
    '{"destination": "express", "weight": 15}'
    '{"destination": "express", "weight": 25}'
    '{"destination": "international", "weight": 5}'
    '{"destination": "international", "weight": 15}'
    '{"destination": "international", "weight": 25}'
)

shipping_labels=(
    "Standard, 5kg"
    "Standard, 15kg"
    "Standard, 25kg"
    "Express, 5kg"
    "Express, 15kg"
    "Express, 25kg"
    "International, 5kg"
    "International, 15kg"
    "International, 25kg"
)

for i in "${!shipping_tests[@]}"; do
    echo -n "  ${shipping_labels[$i]} -> "
    execute_rule "shipping_calculator" "${shipping_tests[$i]}" "SHIPPING_CALCULATED"
done
echo ""

# ============================================================
# Summary
# ============================================================
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${YELLOW}Test Summary${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "  Total Requests: ${TOTAL_REQUESTS}"
echo -e "  ${GREEN}Successful: ${SUCCESS_COUNT}${NC}"
echo -e "  ${RED}Failed: ${FAIL_COUNT}${NC}"

if [ $FAIL_COUNT -eq 0 ]; then
    echo -e "\n  ${GREEN}✓ All tests passed!${NC}"
else
    echo -e "\n  ${RED}✗ Some tests failed${NC}"
fi
echo ""

# ============================================================
# Metrics Report
# ============================================================
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"
echo -e "${YELLOW}Metrics Report${NC}"
echo -e "${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${NC}"

metrics=$(curl -s "${SERVER_URL}/metrics")

echo -e "\n${CYAN}[System Metrics]${NC}"
echo "$metrics" | grep -E "^ordo_(info|uptime|rules_total|executions_total)" | while read line; do
    echo "  $line"
done

echo -e "\n${CYAN}[Execution Duration (histogram)]${NC}"
echo "$metrics" | grep "ordo_execution_duration_seconds_count" | while read line; do
    echo "  $line"
done

echo -e "\n${CYAN}[Custom Rule Metrics]${NC}"
echo "$metrics" | grep "^ordo_rule_" | while read line; do
    echo "  $line"
done

echo ""
echo -e "${CYAN}╔══════════════════════════════════════════════════════════════╗${NC}"
echo -e "${CYAN}║                    Test Complete!                            ║${NC}"
echo -e "${CYAN}╚══════════════════════════════════════════════════════════════╝${NC}"
