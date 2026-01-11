import { ref } from 'vue';
import { trackEvent, AnalyticsEvents } from '../utils/analytics';

export interface TourStep {
  element: string;
  popover: {
    title: string;
    description: string;
    side?: 'top' | 'right' | 'bottom' | 'left';
    align?: 'start' | 'center' | 'end';
  };
  onHighlightStarted?: () => void;
}

export interface TourOptions {
  onSwitchToFlow?: () => void;
}

const TOUR_COMPLETED_KEY = 'ordo-tour-completed';

// Helper to wait for element to appear
function waitForElement(selector: string, timeout = 2000): Promise<Element | null> {
  return new Promise((resolve) => {
    const element = document.querySelector(selector);
    if (element) {
      resolve(element);
      return;
    }

    const observer = new MutationObserver(() => {
      const el = document.querySelector(selector);
      if (el) {
        observer.disconnect();
        resolve(el);
      }
    });

    observer.observe(document.body, { childList: true, subtree: true });

    setTimeout(() => {
      observer.disconnect();
      resolve(document.querySelector(selector));
    }, timeout);
  });
}

export function useTour(options: TourOptions = {}) {
  const isTourActive = ref(false);
  const currentStep = ref(0);
  const driverInstance = ref<any>(null);

  const tourSteps: TourStep[] = [
    {
      element: '[data-tour="explorer"]',
      popover: {
        title: 'File Explorer',
        description: 'Browse and manage your rule files here. Click on a file to open it in the editor.',
        side: 'right',
        align: 'start',
      },
    },
    {
      element: '[data-tour="mode-form"]',
      popover: {
        title: 'Form Mode',
        description: 'Edit rules using a structured form interface. Great for detailed configuration.',
        side: 'right',
      },
    },
    {
      element: '[data-tour="mode-flow"]',
      popover: {
        title: 'Flow Mode',
        description: 'Visualize your rules as a flow chart. Drag to rearrange, click nodes to edit.',
        side: 'right',
      },
      onHighlightStarted: () => {
        // Switch to flow mode when this step is highlighted
        options.onSwitchToFlow?.();
      },
    },
    {
      element: '[data-tour="editor"]',
      popover: {
        title: 'Rule Editor',
        description: 'This is where you design your business rules. Try switching between Form and Flow modes!',
        side: 'left',
        align: 'center',
      },
    },
    {
      element: '[data-tour="console"]',
      popover: {
        title: 'Execution Console',
        description: 'Click here to open the execution panel. Test your rules with sample data and see results instantly!',
        side: 'top',
      },
    },
    {
      element: '[data-tour="json-output"]',
      popover: {
        title: 'JSON Output',
        description: 'View the serialized JSON of your ruleset. Copy it to use in your application.',
        side: 'left',
      },
    },
  ];

  async function startTour() {
    // Dynamically import driver.js
    const { driver } = await import('driver.js');
    await import('driver.js/dist/driver.css');

    driverInstance.value = driver({
      showProgress: true,
      showButtons: ['next', 'previous', 'close'],
      animate: true,
      allowClose: true,
      overlayColor: 'rgba(0, 0, 0, 0.75)',
      stagePadding: 8,
      stageRadius: 8,
      progressText: '{{current}} / {{total}}',
      onDestroyStarted: () => {
        driverInstance.value?.destroy();
        isTourActive.value = false;
        localStorage.setItem(TOUR_COMPLETED_KEY, 'true');
        trackEvent(AnalyticsEvents.TOUR_COMPLETED);
      },
      onHighlightStarted: (_element: any, step: any) => {
        const stepIndex = tourSteps.findIndex(s => s.element === step.element);
        if (stepIndex !== -1 && tourSteps[stepIndex].onHighlightStarted) {
          tourSteps[stepIndex].onHighlightStarted!();
        }
      },
      steps: tourSteps.map(step => ({
        element: step.element,
        popover: {
          title: step.popover.title,
          description: step.popover.description,
          side: step.popover.side,
          align: step.popover.align,
        },
      })),
    });

    isTourActive.value = true;
    driverInstance.value.drive();
    trackEvent(AnalyticsEvents.TOUR_STARTED);
  }

  function shouldShowTour(): boolean {
    return localStorage.getItem(TOUR_COMPLETED_KEY) !== 'true';
  }

  function resetTour() {
    localStorage.removeItem(TOUR_COMPLETED_KEY);
  }

  return {
    isTourActive,
    currentStep,
    startTour,
    shouldShowTour,
    resetTour,
  };
}
