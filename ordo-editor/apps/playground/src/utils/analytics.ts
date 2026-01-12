/**
 * Analytics Module - PostHog Integration
 *
 * This module handles all analytics tracking for the playground.
 * PostHog is loaded asynchronously to avoid blocking the main thread.
 */

declare global {
  interface Window {
    posthog: any;
  }
}

const POSTHOG_KEY = 'phc_BCRuie4xhbSduEL471w7XvQyPcP14QBXPidqdHYf4VY';
const POSTHOG_HOST = 'https://us.i.posthog.com';

let initialized = false;

/**
 * Initialize PostHog analytics
 */
export function initAnalytics(): void {
  if (initialized || typeof window === 'undefined') return;

  // Skip in development mode (optional - remove if you want to track in dev)
  // if (import.meta.env.DEV) {
  //   console.log('[Analytics] Skipped in development mode');
  //   return;
  // }

  try {
    // PostHog snippet
    !(function (t: Document, e: any) {
      var o: string, n: number, p: HTMLScriptElement, r: HTMLScriptElement | null;
      if (!e.__SV && !(window.posthog && window.posthog.__loaded)) {
        window.posthog = e;
        e._i = [];
        e.init = function (i: string, s: any, a?: string) {
          function g(t: any, e: string) {
            var o = e.split('.');
            if (2 == o.length) {
              t = t[o[0]];
              e = o[1];
            }
            t[e] = function () {
              t.push([e].concat(Array.prototype.slice.call(arguments, 0)));
            };
          }
          p = t.createElement('script') as HTMLScriptElement;
          p.type = 'text/javascript';
          p.crossOrigin = 'anonymous';
          p.async = true;
          p.src =
            s.api_host.replace('.i.posthog.com', '-assets.i.posthog.com') + '/static/array.js';
          r = t.getElementsByTagName('script')[0] as HTMLScriptElement;
          r.parentNode?.insertBefore(p, r);

          var u = e;
          if (void 0 !== a) {
            u = e[a] = [];
          } else {
            a = 'posthog';
          }
          u.people = u.people || [];
          u.toString = function (t: boolean) {
            var e = 'posthog';
            if ('posthog' !== a) e += '.' + a;
            if (!t) e += ' (stub)';
            return e;
          };
          u.people.toString = function () {
            return u.toString(1) + '.people (stub)';
          };

          const methods =
            'init ts ns yi rs os Qr es capture Hi calculateEventProperties hs register register_once register_for_session unregister unregister_for_session fs getFeatureFlag getFeatureFlagPayload isFeatureEnabled reloadFeatureFlags updateFlags updateEarlyAccessFeatureEnrollment getEarlyAccessFeatures on onFeatureFlags onSurveysLoaded onSessionId getSurveys getActiveMatchingSurveys renderSurvey displaySurvey cancelPendingSurvey canRenderSurvey canRenderSurveyAsync identify setPersonProperties group resetGroups setPersonPropertiesForFlags resetPersonPropertiesForFlags setGroupPropertiesForFlags resetGroupPropertiesForFlags reset get_distinct_id getGroups get_session_id get_session_replay_url alias set_config startSessionRecording stopSessionRecording sessionRecordingStarted captureException startExceptionAutocapture stopExceptionAutocapture loadToolbar get_property getSessionProperty vs us createPersonProfile cs Yr ps opt_in_capturing opt_out_capturing has_opted_in_capturing has_opted_out_capturing get_explicit_consent_status is_capturing clear_opt_in_out_capturing ls debug O ds getPageViewId captureTraceFeedback captureTraceMetric Vr'.split(
              ' '
            );

          for (n = 0; n < methods.length; n++) {
            g(u, methods[n]);
          }
          e._i.push([i, s, a]);
        };
        e.__SV = 1;
      }
    })(document, window.posthog || []);

    // Initialize PostHog
    window.posthog.init(POSTHOG_KEY, {
      api_host: POSTHOG_HOST,
      person_profiles: 'identified_only',
      capture_pageview: true,
      capture_pageleave: true,
    });

    initialized = true;
    console.log('[Analytics] PostHog initialized');
  } catch (error) {
    console.error('[Analytics] Failed to initialize PostHog:', error);
  }
}

/**
 * Track a custom event
 */
export function trackEvent(eventName: string, properties?: Record<string, any>): void {
  if (!initialized || typeof window === 'undefined') return;

  try {
    window.posthog?.capture(eventName, properties);
  } catch (error) {
    console.error('[Analytics] Failed to track event:', error);
  }
}

/**
 * Identify a user
 */
export function identifyUser(userId: string, properties?: Record<string, any>): void {
  if (!initialized || typeof window === 'undefined') return;

  try {
    window.posthog?.identify(userId, properties);
  } catch (error) {
    console.error('[Analytics] Failed to identify user:', error);
  }
}

/**
 * Reset analytics (on logout)
 */
export function resetAnalytics(): void {
  if (!initialized || typeof window === 'undefined') return;

  try {
    window.posthog?.reset();
  } catch (error) {
    console.error('[Analytics] Failed to reset:', error);
  }
}

// Predefined event names for consistency
export const AnalyticsEvents = {
  // Tour events
  TOUR_STARTED: 'tour_started',
  TOUR_COMPLETED: 'tour_completed',
  TOUR_SKIPPED: 'tour_skipped',
  TOUR_STEP_VIEWED: 'tour_step_viewed',

  // Editor events
  FILE_CREATED: 'file_created',
  FILE_DELETED: 'file_deleted',
  MODE_SWITCHED: 'mode_switched',

  // Execution events
  RULE_EXECUTED: 'rule_executed',
  EXECUTION_SUCCESS: 'execution_success',
  EXECUTION_ERROR: 'execution_error',

  // UI events
  THEME_CHANGED: 'theme_changed',
  LOCALE_CHANGED: 'locale_changed',
  JSON_COPIED: 'json_copied',
} as const;
