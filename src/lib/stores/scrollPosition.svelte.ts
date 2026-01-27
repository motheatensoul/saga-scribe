/**
 * Store for preserving scroll positions across tab switches in the preview panel.
 */

type ViewMode = 'xml' | 'rendered' | 'xslt';

interface ScrollState {
    scrollTop: number;
    scrollLeft: number;
}

function createScrollPositionStore() {
    let _positions = $state<Record<ViewMode, ScrollState>>({
        xml: { scrollTop: 0, scrollLeft: 0 },
        rendered: { scrollTop: 0, scrollLeft: 0 },
        xslt: { scrollTop: 0, scrollLeft: 0 },
    });

    return {
        getPosition(view: ViewMode): ScrollState {
            return _positions[view];
        },

        savePosition(view: ViewMode, scrollTop: number, scrollLeft: number = 0) {
            _positions[view] = { scrollTop, scrollLeft };
        },

        clear() {
            _positions = {
                xml: { scrollTop: 0, scrollLeft: 0 },
                rendered: { scrollTop: 0, scrollLeft: 0 },
                xslt: { scrollTop: 0, scrollLeft: 0 },
            };
        },
    };
}

export const scrollPositionStore = createScrollPositionStore();
