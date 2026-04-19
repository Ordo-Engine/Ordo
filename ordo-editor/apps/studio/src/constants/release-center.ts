import type { ReleaseExecutionStatus, ReleaseInstanceStatus, ReleaseRequestStatus, RolloutStrategy } from '@/api/types'

export function labelReleaseRequestStatus(status: ReleaseRequestStatus) {
  return status
}

export function labelReleaseExecutionStatus(status: ReleaseExecutionStatus) {
  return status
}

export function labelInstanceStatus(status: ReleaseInstanceStatus) {
  return status
}

export function labelRolloutStrategy(strategy: RolloutStrategy) {
  switch (strategy.kind) {
    case 'all_at_once':
      return 'All at once'
    case 'fixed_batch':
      return `${strategy.batch_size ?? 0} instances / batch`
    case 'percentage_batch':
      return `${strategy.batch_percentage ?? 0}% / batch`
    case 'time_interval_batch':
      return `${strategy.batch_size ?? 0} instances every ${strategy.batch_interval_seconds ?? 0}s`
    default:
      return strategy.kind ?? '—'
  }
}
