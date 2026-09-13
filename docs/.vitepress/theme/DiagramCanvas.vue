<script setup lang="ts">
/**
 * A pannable, zoomable frame for a diagram.
 *
 * Diagrams outgrow the column they sit in — a flowchart with a loop is
 * wider than prose, and shrinking it to fit makes the labels unreadable.
 * So the frame is a fixed viewport and the diagram moves inside it.
 *
 * Everything it does is reachable three ways: pointer, buttons, and
 * keyboard. The keyboard is not an afterthought here — a diagram that
 * can only be explored by dragging is a diagram some readers cannot
 * explore at all.
 */
import { ref, onMounted, onBeforeUnmount, computed, watchEffect, nextTick } from 'vue';

const MIN = 0.4;
const MAX = 4;
const STEP = 1.15;

const frame = ref<HTMLElement | null>(null);
const scale = ref(1);
const x = ref(0);
const y = ref(0);
const dragging = ref(false);
const moved = ref(false);

let startX = 0;
let startY = 0;
let originX = 0;
let originY = 0;

// Pan is a transform; zoom is not.
//
// `transform: scale()` on a composited layer rasterises the diagram once
// at 1x and then stretches that bitmap, so a vector drawing goes soft the
// moment it is enlarged. Resizing the SVG itself instead makes the
// browser re-render the vectors at the new size, which is the whole
// reason for shipping an SVG.
const transform = computed(() => `translate(${x.value}px, ${y.value}px)`);

const stage = ref<HTMLElement | null>(null);
const svg = ref<SVGSVGElement | null>(null);
const baseWidth = ref(0);
// What "1" means. A diagram wider than the frame opens fitted to it, so
// the first thing the reader sees is the whole picture; zoom is relative
// to that, not to the SVG's intrinsic size.
const fit = ref(1);
// The frame takes the diagram's shape rather than imposing one.
//
// Measured, these three diagrams run from 0.59 (tall) to 2.81 (wide)
// while a fixed frame was 1.47 — so one fixed height either marooned a
// wide diagram in empty space or squeezed a tall one to nothing.
const frameHeight = ref<string | null>(null);
const MIN_H = 200;
const MAX_H = 620;

/** Mermaid renders after mount, so the SVG has to be waited for. */
function captureSvg() {
    const el = stage.value?.querySelector('svg') as SVGSVGElement | null;
    if (!el || el === svg.value) return;
    svg.value = el;

    const box = el.viewBox?.baseVal;
    const w = box?.width || el.getBoundingClientRect().width || 0;
    const h = box?.height || el.getBoundingClientRect().height || 0;
    if (!w) return;
    baseWidth.value = w;

    if (h) {
        const width = frame.value?.getBoundingClientRect().width ?? 0;
        const wanted = width ? width / (w / h) : 0;
        if (wanted) {
            frameHeight.value = `${Math.round(Math.min(MAX_H, Math.max(MIN_H, wanted)))}px`;
        }
    }

    // After the height lands, so the fit is measured against the frame
    // the reader will actually see.
    void nextTick(() => measureFit(w, h));
}

function measureFit(w: number, h: number) {
    const frameBox = frame.value?.getBoundingClientRect();
    if (frameBox && h) {
        // Fills the frame in both directions, up or down. Capping this
        // at 1 left a small diagram marooned in the middle of a large
        // empty box — the reader has to zoom before they can read a
        // picture that had room to be legible from the start.
        const margin = 48;
        fit.value = Math.min(
            3,
            (frameBox.width - margin) / w,
            (frameBox.height - margin) / h
        );
    }
    scale.value = fit.value;
}

watchEffect(() => {
    const el = svg.value;
    if (!el || !baseWidth.value) return;
    el.style.width = `${baseWidth.value * scale.value}px`;
    el.style.maxWidth = 'none';
    el.style.height = 'auto';
});

const untouched = computed(
    () => scale.value === fit.value && x.value === 0 && y.value === 0
);

function clamp(v: number) {
    return Math.min(MAX * fit.value, Math.max(MIN * fit.value, v));
}

/** Zoom about a point, so what is under the cursor stays under it. */
function zoomAt(factor: number, clientX?: number, clientY?: number) {
    const next = clamp(scale.value * factor);
    if (next === scale.value) return;

    const box = frame.value?.getBoundingClientRect();
    if (box && clientX !== undefined && clientY !== undefined) {
        const px = clientX - box.left - box.width / 2;
        const py = clientY - box.top - box.height / 2;
        const ratio = next / scale.value;
        x.value = px - (px - x.value) * ratio;
        y.value = py - (py - y.value) * ratio;
    }
    scale.value = next;
}

function onWheel(e: WheelEvent) {
    // Only once the reader has said this frame is what they are using.
    // Hijacking the wheel on hover traps somebody who was scrolling
    // past on their way down the page.
    if (!active.value && !e.ctrlKey && !e.metaKey) return;
    e.preventDefault();
    zoomAt(e.deltaY < 0 ? STEP : 1 / STEP, e.clientX, e.clientY);
}

const active = ref(false);

function onPointerDown(e: PointerEvent) {
    if (e.button !== 0) return;
    dragging.value = true;
    moved.value = false;
    startX = e.clientX;
    startY = e.clientY;
    originX = x.value;
    originY = y.value;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
}

function onPointerMove(e: PointerEvent) {
    if (!dragging.value) return;
    const dx = e.clientX - startX;
    const dy = e.clientY - startY;
    if (Math.abs(dx) > 3 || Math.abs(dy) > 3) moved.value = true;
    x.value = originX + dx;
    y.value = originY + dy;
}

function onPointerUp(e: PointerEvent) {
    if (!dragging.value) return;
    dragging.value = false;
    (e.currentTarget as HTMLElement).releasePointerCapture(e.pointerId);
    // A click that did not drag is the reader saying "I am using this",
    // which is what turns the wheel into a zoom.
    if (!moved.value) active.value = true;
}

function reset() {
    scale.value = fit.value;
    x.value = 0;
    y.value = 0;
}

function onKey(e: KeyboardEvent) {
    const pan = e.shiftKey ? 80 : 24;
    const keys: Record<string, () => void> = {
        ArrowLeft: () => (x.value += pan),
        ArrowRight: () => (x.value -= pan),
        ArrowUp: () => (y.value += pan),
        ArrowDown: () => (y.value -= pan),
        '+': () => zoomAt(STEP),
        '=': () => zoomAt(STEP),
        '-': () => zoomAt(1 / STEP),
        '0': reset,
        Escape: () => {
            reset();
            active.value = false;
        }
    };
    const run = keys[e.key];
    if (!run) return;
    e.preventDefault();
    run();
}

function onBlur() {
    active.value = false;
}

let observer: MutationObserver | null = null;

onMounted(() => {
    // Attached by hand because Vue's `@wheel` is passive by default, and
    // a passive listener cannot call preventDefault — the page would
    // scroll underneath the zoom.
    frame.value?.addEventListener('wheel', onWheel, { passive: false });
    captureSvg();
    if (stage.value) {
        observer = new MutationObserver(captureSvg);
        observer.observe(stage.value, { childList: true, subtree: true });
    }
});

onBeforeUnmount(() => {
    frame.value?.removeEventListener('wheel', onWheel);
    observer?.disconnect();
});
</script>

<template>
    <figure class="diagram-canvas">
        <div
            ref="frame"
            class="frame"
            :class="{ dragging, active }"
            :style="frameHeight ? { height: frameHeight } : undefined"
            tabindex="0"
            role="img"
            aria-label="Diagram. Drag to pan. Click it, then use the wheel to zoom. Arrow keys pan, plus and minus zoom, zero resets."
            @pointerdown="onPointerDown"
            @pointermove="onPointerMove"
            @pointerup="onPointerUp"
            @pointercancel="onPointerUp"
            @keydown="onKey"
            @blur="onBlur"
        >
            <div ref="stage" class="stage" :style="{ transform }">
                <slot />
            </div>

            <div class="controls" aria-hidden="true" @pointerdown.stop @pointerup.stop @click.stop>
                <button type="button" title="Zoom out" @click="zoomAt(1 / STEP)">−</button>
                <button type="button" title="Zoom in" @click="zoomAt(STEP)">+</button>
                <button type="button" title="Reset" :disabled="untouched" @click="reset">⟲</button>
            </div>

            <p class="hint" aria-hidden="true">
                {{ active ? 'wheel to zoom · drag to pan · esc to release' : 'click to zoom · drag to pan' }}
            </p>
        </div>
    </figure>
</template>

<style scoped>
.diagram-canvas {
    margin: 1.5rem 0;
}

.frame {
    position: relative;
    overflow: hidden;
    /* A starting size only — replaced once the diagram has been measured
     * and its own proportions are known. */
    height: clamp(14rem, 40vh, 26rem);
    border: 1px solid var(--vp-c-divider);
    border-radius: 10px;
    background: var(--vp-c-bg-soft);
    cursor: grab;
    touch-action: none;
    /* Dragging a diagram must not sweep a selection across its labels:
     * the highlight follows the pointer and the drag reads as broken. */
    user-select: none;
    -webkit-user-select: none;
    display: grid;
    place-items: center;
}

.frame.dragging {
    cursor: grabbing;
}

/* The accent marks "this frame now owns the wheel" — a state the
 * reader opted into, so it should be visible that they did. */
.frame.active {
    border-color: var(--arex-primary);
}

.frame:focus-visible {
    outline: 2px solid var(--arex-primary);
    outline-offset: 2px;
}

.stage {
    /* No transition: a dragged element that eases lags behind the cursor,
     * which reads as the page being slow.
     *
     * No `will-change` either. It promotes this to its own compositor
     * layer, and a promoted layer is rasterised once and then scaled as a
     * bitmap — which turned the diagram to mush the first time it was
     * enlarged. Zoom resizes the SVG instead; see the script. */
    transform-origin: center center;
}

.stage :deep(svg) {
    max-width: none !important;
    height: auto;
}

.controls {
    position: absolute;
    right: 0.6rem;
    bottom: 0.6rem;
    display: flex;
    gap: 0.35rem;
}

.controls button {
    width: 2rem;
    height: 2rem;
    border: 1px solid var(--vp-c-divider);
    border-radius: 6px;
    background: var(--vp-c-bg);
    color: var(--vp-c-text-1);
    font-size: 0.95rem;
    line-height: 1;
    cursor: pointer;
    transition: border-color 0.15s ease, color 0.15s ease;
}

.controls button:hover:not(:disabled) {
    border-color: var(--arex-primary);
    color: var(--arex-primary-text);
}

.controls button:disabled {
    opacity: 0.4;
    cursor: default;
}

.hint {
    position: absolute;
    left: 0.75rem;
    bottom: 0.75rem;
    margin: 0;
    font-size: 0.72rem;
    color: var(--vp-c-text-3);
    pointer-events: none;
    user-select: none;
}

@media (prefers-reduced-motion: reduce) {
    .controls button {
        transition: none;
    }
}
</style>
