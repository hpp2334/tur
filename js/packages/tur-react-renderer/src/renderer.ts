import React from "react";
import ReactReconciler from "react-reconciler";
import type { TurNodeHandle } from "./tur";

const ctx = __tur.__ctx;

const creators: Record<string, () => TurNodeHandle> = {
    tur_flex: () => __tur.createFlex(ctx),
    tur_flex_item: () => __tur.createFlexItem(ctx),
    tur_stack: () => __tur.createStack(ctx),
    tur_positioned: () => __tur.createPositioned(ctx),
    tur_container: () => __tur.createContainer(ctx),
    tur_paragraph: () => __tur.createParagraph(ctx),
    tur_pointer_interact: () => __tur.createPointerInteract(ctx),
    tur_focusable: () => __tur.createFocusable(ctx),
    tur_editable_text: () => __tur.createEditableText(ctx, null as never),
    tur_image: () => __tur.createImage(ctx),
    tur_scroll_view: () => __tur.createScrollView(ctx),
};

type Props = Record<string, unknown>;

export interface TurInstance {
    handle: TurNodeHandle;
    type: string;
    props: Props;
}

function setProps(
    handle: TurNodeHandle,
    props: Props,
    previousProps: Props | null,
) {
    for (const key in props) {
        if (key === "children" || key === "key" || key === "ref") continue;
        const value = props[key];
        const prev = previousProps?.[key];
        if (value === null || value === undefined) {
            if (prev !== null && prev !== undefined) {
                __tur.setAttribute(ctx, handle, key, null);
            }
        } else {
            __tur.setAttribute(ctx, handle, key, value);
        }
    }
}

let updatePriority = 0;

const reconciler = ReactReconciler({
    supportsMutation: true,
    supportsPersistence: false,
    supportsHydration: false,

    createInstance(type: string, props: Props): TurInstance {
        const create = creators[type];
        if (!create) throw new Error(`unknown element type: ${type}`);
        const handle = create();
        setProps(handle, props, null);
        return { handle, type, props };
    },

    createTextInstance(): never {
        throw new Error("text instances not supported; use spans prop");
    },

    appendInitialChild(parentInstance: TurInstance, child: TurInstance): void {
        __tur.appendChild(ctx, parentInstance.handle, child.handle);
    },

    finalizeInitialChildren(): boolean {
        return false;
    },

    commitUpdate(
        instance: TurInstance,
        _type: string,
        _prevProps: Props,
        nextProps: Props,
        _internalHandle: unknown,
    ): void {
        setProps(instance.handle, nextProps, _prevProps);
        instance.props = nextProps;
    },

    appendChild(parentInstance: TurInstance, child: TurInstance): void {
        __tur.appendChild(ctx, parentInstance.handle, child.handle);
    },

    appendChildToContainer(container: TurNodeHandle, child: TurInstance): void {
        __tur.appendChild(ctx, container, child.handle);
    },

    insertBefore(
        parentInstance: TurInstance,
        child: TurInstance,
        before: TurInstance,
    ): void {
        __tur.insertBefore(
            ctx,
            parentInstance.handle,
            child.handle,
            before.handle,
        );
    },

    insertInContainerBefore(
        container: TurNodeHandle,
        child: TurInstance,
        before: TurInstance,
    ): void {
        __tur.insertBefore(ctx, container, child.handle, before.handle);
    },

    removeChild(parentInstance: TurInstance, child: TurInstance): void {
        __tur.removeChild(ctx, parentInstance.handle, child.handle);
    },

    removeChildFromContainer(
        container: TurNodeHandle,
        child: TurInstance,
    ): void {
        __tur.removeChild(ctx, container, child.handle);
    },

    getRootHostContext() {
        return {};
    },

    getChildHostContext(parentHostContext: Record<string, unknown>) {
        return parentHostContext;
    },

    shouldSetTextContent(): boolean {
        return false;
    },

    getPublicInstance(instance: TurInstance): TurNodeHandle {
        return instance.handle;
    },

    prepareForCommit(): null {
        return null;
    },

    resetAfterCommit(): void {},

    clearContainer(): void {},

    scheduleTimeout(
        fn: (...args: unknown[]) => unknown,
        delay?: number,
    ): unknown {
        return setTimeout(fn, delay);
    },

    cancelTimeout(id: unknown): void {
        clearTimeout(id as ReturnType<typeof setTimeout>);
    },

    noTimeout: undefined,
    isPrimaryRenderer: true,
    supportsMicrotasks: typeof queueMicrotask === "function",
    scheduleMicrotask:
        typeof queueMicrotask === "function"
            ? (cb: () => void) => queueMicrotask(cb)
            : (undefined as never),
    getCurrentUpdatePriority: () => updatePriority,
    setCurrentUpdatePriority(p: number): void {
        updatePriority = p;
    },
    resolveUpdatePriority(): number {
        return updatePriority || 2;
    },
    shouldAttemptEagerTransition(): boolean {
        return false;
    },
    maySuspendCommit(): boolean {
        return false;
    },
    preloadInstance(): boolean {
        return false;
    },
    startSuspendingCommit(): void {},
    suspendInstance(): never {
        throw new Error("suspendInstance not supported");
    },
    waitForCommitToBeReady(): never {
        throw new Error("waitForCommitToBeReady not supported");
    },
    NotPendingTransition: null,
    HostTransitionContext: {
        $$typeof: Symbol.for("react.context"),
        Provider: null as never,
        Consumer: null as never,
        _currentValue: null,
        _currentValue2: null,
        _threadCount: 0,
    },
    resetFormInstance(): void {},
    getInstanceFromNode: () => null,
    beforeActiveInstanceBlur() {},
    afterActiveInstanceBlur() {},
    preparePortalMount() {},
    detachDeletedInstance() {},
    prepareScopeUpdate() {},
    getInstanceFromScope(): null {
        return null;
    },
    // biome-ignore lint/suspicious/noExplicitAny: react-reconciler HostConfig requires full any cast
} as any);

let _container: ReturnType<typeof reconciler.createContainer> | null = null;

export function renderRoot(component: React.ComponentType): TurNodeHandle {
    const root = __tur.createRoot(ctx);
    const handleError = (error: unknown) => {
        console.error(error);
    };
    _container = reconciler.createContainer(
        root,
        0,
        null,
        false,
        null,
        "",
        handleError,
        handleError,
        handleError,
        () => {},
    );

    const element = React.createElement(component);
    // biome-ignore lint/suspicious/noExplicitAny: flushSyncFromReconciler is an internal API
    (reconciler as any).flushSyncFromReconciler(() => {
        reconciler.updateContainer(element, _container, null, () => {});
    });
    return root;
}
