#!/bin/bash
# =============================================================================
# Ordo Performance Benchmark Report Generator
# =============================================================================
#
# Usage:
#   ./scripts/bench-report.sh [command]
#
# Commands:
#   run       - Run benchmarks and save as 'current' baseline
#   compare   - Compare 'current' with 'previous' baseline
#   baseline  - Save current results as 'previous' baseline
#   report    - Generate HTML report
#   all       - Run all benchmarks and generate report
#
# Requirements:
#   - Rust toolchain with cargo
#   - criterion (included in dev-dependencies)

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BENCH_DIR="$PROJECT_ROOT/crates/ordo-core"
REPORT_DIR="$PROJECT_ROOT/target/criterion"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

print_header() {
    echo ""
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${BLUE}  $1${NC}"
    echo -e "${BLUE}═══════════════════════════════════════════════════════════════${NC}"
    echo ""
}

print_info() {
    echo -e "${GREEN}[INFO]${NC} $1"
}

print_warn() {
    echo -e "${YELLOW}[WARN]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Get system info
get_system_info() {
    print_header "System Information"
    
    echo "Platform: $(uname -s) $(uname -m)"
    echo "Kernel: $(uname -r)"
    
    if [[ "$(uname -s)" == "Darwin" ]]; then
        echo "macOS: $(sw_vers -productVersion)"
        echo "CPU: $(sysctl -n machdep.cpu.brand_string)"
        echo "Cores: $(sysctl -n hw.ncpu)"
        echo "Memory: $(( $(sysctl -n hw.memsize) / 1024 / 1024 / 1024 )) GB"
    else
        echo "CPU: $(grep 'model name' /proc/cpuinfo | head -1 | cut -d: -f2 | xargs)"
        echo "Cores: $(nproc)"
        echo "Memory: $(free -h | grep Mem | awk '{print $2}')"
    fi
    
    echo "Rust: $(rustc --version)"
    echo "Cargo: $(cargo --version)"
    echo ""
}

# Run unified benchmark
run_benchmark() {
    local baseline_name="${1:-current}"
    
    print_header "Running Unified Benchmark Suite"
    print_info "Baseline name: $baseline_name"
    print_info "This may take several minutes..."
    echo ""
    
    cd "$BENCH_DIR"
    
    cargo bench --bench unified_bench -- --save-baseline "$baseline_name" 2>&1 | tee /tmp/bench_output.txt
    
    print_info "Benchmark completed!"
    echo ""
}

# Compare baselines
compare_baselines() {
    local baseline="${1:-previous}"
    
    print_header "Comparing with Baseline: $baseline"
    
    cd "$BENCH_DIR"
    
    if [ ! -d "$REPORT_DIR" ]; then
        print_error "No benchmark results found. Run benchmarks first."
        exit 1
    fi
    
    cargo bench --bench unified_bench -- --baseline "$baseline" 2>&1 | tee /tmp/bench_compare.txt
    
    echo ""
}

# Save current as previous
save_baseline() {
    print_header "Saving Current as Previous Baseline"
    
    cd "$BENCH_DIR"
    
    # Run benchmark with baseline save
    cargo bench --bench unified_bench -- --save-baseline previous 2>&1 | tail -20
    
    print_info "Baseline saved as 'previous'"
}

# Generate summary report
generate_report() {
    print_header "Performance Summary Report"
    
    local report_file="$PROJECT_ROOT/BENCHMARK_REPORT.md"
    
    cat > "$report_file" << 'EOF'
# Ordo Performance Benchmark Report

## Test Environment

EOF

    # Add system info
    echo "- **Platform**: $(uname -s) $(uname -m)" >> "$report_file"
    echo "- **Rust**: $(rustc --version | cut -d' ' -f2)" >> "$report_file"
    echo "- **Date**: $(date -u +"%Y-%m-%d %H:%M:%S UTC")" >> "$report_file"
    
    if [[ "$(uname -s)" == "Darwin" ]]; then
        echo "- **CPU**: $(sysctl -n machdep.cpu.brand_string)" >> "$report_file"
        echo "- **Cores**: $(sysctl -n hw.ncpu)" >> "$report_file"
    fi
    
    cat >> "$report_file" << 'EOF'

## Benchmark Results

### Key Performance Indicators

| Metric | Value | Notes |
|--------|-------|-------|
EOF

    # Parse benchmark results if available
    if [ -f /tmp/bench_output.txt ]; then
        # Extract key metrics
        local simple_rule=$(grep -A2 "minimal_compiled" /tmp/bench_output.txt 2>/dev/null | grep "time:" | head -1 | awk '{print $2, $3}' || echo "N/A")
        local throughput=$(grep -A2 "batch_1k" /tmp/bench_output.txt 2>/dev/null | grep "thrpt:" | head -1 | awk '{print $2, $3}' || echo "N/A")
        local len_func=$(grep -A2 "len_string" /tmp/bench_output.txt 2>/dev/null | grep "time:" | head -1 | awk '{print $2, $3}' || echo "N/A")
        
        echo "| Simple Rule Execution | $simple_rule | Compiled, single decision |" >> "$report_file"
        echo "| Throughput (1K batch) | $throughput | Sequential execution |" >> "$report_file"
        echo "| len() function | $len_func | Fast-path optimized |" >> "$report_file"
    else
        echo "| *Run benchmarks first* | - | - |" >> "$report_file"
    fi

    cat >> "$report_file" << 'EOF'

### Benchmark Categories

#### 1. Expression Parsing
Time to parse expression strings into AST.

#### 2. Expression Evaluation  
Time to evaluate pre-parsed expressions against context.

#### 3. Rule Execution
End-to-end rule execution including condition evaluation and output generation.

#### 4. Built-in Functions
Individual function call performance (optimized fast-path).

#### 5. Initialization
Component creation overhead.

#### 6. Throughput
Batch processing performance (executions per second).

#### 7. Scaling
Performance with increasing rule complexity.

## Running Benchmarks

```bash
# Run all benchmarks
cargo bench --bench unified_bench

# Run specific group
cargo bench --bench unified_bench -- "rule_execution"

# Compare with baseline
cargo bench --bench unified_bench -- --baseline previous

# Save baseline
cargo bench --bench unified_bench -- --save-baseline my_baseline
```

## HTML Reports

Detailed HTML reports are available at:
`target/criterion/report/index.html`

EOF

    print_info "Report generated: $report_file"
    
    # Also check for HTML report
    if [ -f "$REPORT_DIR/report/index.html" ]; then
        print_info "HTML report available: $REPORT_DIR/report/index.html"
    fi
}

# Main
main() {
    local command="${1:-all}"
    
    case "$command" in
        run)
            get_system_info
            run_benchmark "${2:-current}"
            ;;
        compare)
            compare_baselines "${2:-previous}"
            ;;
        baseline)
            save_baseline
            ;;
        report)
            generate_report
            ;;
        all)
            get_system_info
            run_benchmark "current"
            generate_report
            ;;
        help|--help|-h)
            echo "Ordo Performance Benchmark Report Generator"
            echo ""
            echo "Usage: $0 [command] [args]"
            echo ""
            echo "Commands:"
            echo "  run [name]     Run benchmarks, save as baseline (default: current)"
            echo "  compare [name] Compare with baseline (default: previous)"
            echo "  baseline       Save current as 'previous' baseline"
            echo "  report         Generate markdown report"
            echo "  all            Run benchmarks and generate report"
            echo "  help           Show this help"
            ;;
        *)
            print_error "Unknown command: $command"
            echo "Use '$0 help' for usage information"
            exit 1
            ;;
    esac
}

main "$@"
