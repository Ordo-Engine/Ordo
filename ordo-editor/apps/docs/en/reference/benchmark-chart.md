# Interactive Benchmark Chart

<script setup>
import { onMounted, ref } from 'vue'

const chartRef = ref(null)

const DATA = {
  concurrency: [1, 5, 10, 25, 50, 100, 150, 200, 300, 500],
  engines: {
    'Ordo (Rust)':            { qps: [13675,33670,46529,53578,59952,60823,61172,61307,61096,62748], color: '#FF6B35' },
    'OPA (Go)':               { qps: [9478,23547,25768,29616,30930,33747,35713,35629,37029,37082], color: '#4ECDC4' },
    'json-rules-engine (JS)': { qps: [11075,19533,21224,22422,22401,22131,21716,21249,17314,14342], color: '#45B7D1' },
    'Grule (Go)':             { qps: [5199,7282,5983,5721,6445,7523,7960,8371,8679,8680], color: '#96CEB4' },
  }
}

onMounted(async () => {
  if (typeof window === 'undefined') return
  const echarts = await import('echarts')

  // ── QPS Chart ──
  const qpsChart = echarts.init(chartRef.value)

  const series = Object.entries(DATA.engines).map(([name, { qps, color }]) => ({
    name,
    type: 'line',
    data: qps,
    smooth: true,
    symbol: 'circle',
    symbolSize: 8,
    lineStyle: { width: 3, color },
    itemStyle: { color },
    endLabel: {
      show: true,
      formatter: (params) => `${params.seriesName}: ${params.value.toLocaleString()}`,
      fontSize: 12,
      fontWeight: 'bold',
      color: color,
    },
    animationDuration: 3000,
    animationDelay: (idx) => idx * 250,
    animationEasing: 'cubicOut',
    markPoint: name === 'Ordo (Rust)' ? {
      data: [{
        name: 'Saturation',
        coord: [5, 60823],
        symbol: 'pin',
        symbolSize: 50,
        itemStyle: { color: '#FFD700' },
        label: { formatter: '~60K\nSaturation', fontSize: 10, fontWeight: 'bold' }
      }]
    } : undefined,
  }))

  const qpsOption = {
    backgroundColor: 'transparent',
    title: {
      text: 'Rule Engine QPS vs Concurrency',
      subtext: '4-branch decision rule • Apple M1 Pro • 10s per test',
      left: 'center',
      textStyle: { color: '#e0e0e0', fontSize: 18 },
      subtextStyle: { color: '#888', fontSize: 12 },
    },
    tooltip: {
      trigger: 'axis',
      backgroundColor: 'rgba(30,30,50,0.95)',
      borderColor: '#555',
      textStyle: { color: '#fff' },
      formatter: (params) => {
        const conc = DATA.concurrency[params[0].dataIndex]
        let html = `<b>Concurrency: ${conc}</b><br/>`
        params.sort((a, b) => b.value - a.value)
        for (const p of params) {
          html += `${p.marker} ${p.seriesName}: <b>${p.value.toLocaleString()}</b> QPS<br/>`
        }
        // Show gap vs Ordo
        const ordoVal = params.find(p => p.seriesName.includes('Ordo'))?.value
        if (ordoVal) {
          html += '<br/>'
          for (const p of params) {
            if (!p.seriesName.includes('Ordo')) {
              const ratio = (ordoVal / p.value).toFixed(1)
              html += `Ordo is <b>${ratio}x</b> faster than ${p.seriesName.split(' ')[0]}<br/>`
            }
          }
        }
        return html
      },
    },
    legend: {
      bottom: 0,
      textStyle: { color: '#ccc', fontSize: 12 },
      itemWidth: 25,
    },
    grid: {
      left: 80,
      right: 180,
      top: 80,
      bottom: 60,
    },
    xAxis: {
      type: 'category',
      data: DATA.concurrency,
      name: 'Concurrency',
      nameLocation: 'center',
      nameGap: 35,
      nameTextStyle: { color: '#aaa', fontSize: 13 },
      axisLabel: { color: '#aaa' },
      axisLine: { lineStyle: { color: '#555' } },
    },
    yAxis: {
      type: 'value',
      name: 'QPS (requests/sec)',
      nameTextStyle: { color: '#aaa', fontSize: 13 },
      axisLabel: {
        color: '#aaa',
        formatter: (v) => v >= 1000 ? `${v/1000}K` : v,
      },
      splitLine: { lineStyle: { color: '#333', type: 'dashed' } },
      axisLine: { lineStyle: { color: '#555' } },
    },
    series,
  }

  qpsChart.setOption(qpsOption)
  window.addEventListener('resize', () => qpsChart.resize())
})
</script>

<div ref="chartRef" style="width:100%;height:500px;margin:20px 0"></div>

## Data Source

All tests run with [`hey`](https://github.com/rakyll/hey) for 10 seconds per concurrency level on Apple M1 Pro (10-core, 16GB RAM).

| Engine            | C=1    | C=5    | C=10   | C=25   | C=50   | C=100  | C=150  | C=200  | C=300  | C=500  |
| ----------------- | ------ | ------ | ------ | ------ | ------ | ------ | ------ | ------ | ------ | ------ |
| **Ordo**          | 13,675 | 33,670 | 46,529 | 53,578 | 59,952 | 60,823 | 61,172 | 61,307 | 61,096 | 62,748 |
| OPA               | 9,478  | 23,547 | 25,768 | 29,616 | 30,930 | 33,747 | 35,713 | 35,629 | 37,029 | 37,082 |
| json-rules-engine | 11,075 | 19,533 | 21,224 | 22,422 | 22,401 | 22,131 | 21,716 | 21,249 | 17,314 | 14,342 |
| Grule             | 5,199  | 7,282  | 5,983  | 5,721  | 6,445  | 7,523  | 7,960  | 8,371  | 8,679  | 8,680  |

See [full benchmarks](./benchmarks.md) for latency data, distributed mode results, and core engine microbenchmarks.
