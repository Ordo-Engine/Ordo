import { useI18n } from 'vue-i18n';
import type {
  ReleaseExecutionStatus,
  ReleaseInstanceStatus,
  ReleaseRequestStatus,
  RolloutStrategy,
} from '@/api/types';

export function labelReleaseRequestStatus(status: ReleaseRequestStatus) {
  return status;
}

export function labelReleaseExecutionStatus(status: ReleaseExecutionStatus) {
  return status;
}

export function labelInstanceStatus(status: ReleaseInstanceStatus) {
  return status;
}

/**
 * Returns a localised label function for rollout strategies.
 * Must be called inside a Vue component setup context.
 */
export function useRolloutStrategyLabel() {
  const { t } = useI18n();
  return (strategy: RolloutStrategy) => {
    switch (strategy.kind) {
      case 'all_at_once':
        return t('releaseCenter.rolloutStrategyLabel.allAtOnce');
      case 'fixed_batch':
        return t('releaseCenter.rolloutStrategyLabel.fixedBatch', {
          count: strategy.batch_size ?? 0,
        });
      case 'percentage_batch':
        return t('releaseCenter.rolloutStrategyLabel.percentageBatch', {
          pct: strategy.batch_percentage ?? 0,
        });
      case 'time_interval_batch':
        return t('releaseCenter.rolloutStrategyLabel.timeIntervalBatch', {
          count: strategy.batch_size ?? 0,
          seconds: strategy.batch_interval_seconds ?? 0,
        });
      default:
        return strategy.kind ?? '—';
    }
  };
}

/** Legacy alias for non-reactive/non-component contexts (English fallback). */
export function labelRolloutStrategy(strategy: RolloutStrategy) {
  switch (strategy.kind) {
    case 'all_at_once':
      return 'All at once';
    case 'fixed_batch':
      return `${strategy.batch_size ?? 0} instances / batch`;
    case 'percentage_batch':
      return `${strategy.batch_percentage ?? 0}% / batch`;
    case 'time_interval_batch':
      return `${strategy.batch_size ?? 0} instances every ${strategy.batch_interval_seconds ?? 0}s`;
    default:
      return strategy.kind ?? '—';
  }
}
