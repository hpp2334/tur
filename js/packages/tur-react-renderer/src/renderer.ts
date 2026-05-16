import ReactReconciler from "react-reconciler";
import React from "react";
import type { TurNodeHandle } from "./tur";

const ctx = __tur.__ctx;

const creators: Record<string, () => TurNodeHandle> = {
  tur_flex: () => __tur.createFlex(ctx),
  tur_flex_item: () => __tur.createFlexItem(ctx),
  tur_stack: () => __tur.createStack(ctx),
  tur_positioned: () => __tur.createPositioned(ctx),
  tur_container: () => __tur.createContainer(ctx),
  tur_text_container: () => __tur.createTextContainer(ctx),
  tur_text_span: () => __tur.createTextSpan(ctx),
  tur_pointer_interact: () => __tur.createPointerInteract(ctx),
  tur_focusable: () => __tur.createFocusable(ctx),
  tur_input: () => __tur.createInput(ctx),
  tur_image: () => __tur.createImage(ctx),
};

type Props = Record<string, unknown>;

export interface TurInstance {
  handle: TurNodeHandle;
  type: string;
  props: Props;
}

function setProps(handle: TurNodeHandle, props: Props) {
  for (const key in props) {
    if (key === "children" || key === "key" || key === "ref") continue;
    const value = props[key];
    if (value !== null && value !== undefined) {
      __tur.setAttribute(ctx, handle, key, value);
    }
  }
}

let updatePriority = 0;

const reconciler = ReactReconciler(
  {
  supportsMutation: true,
  supportsPersistence: false,
  supportsHydration: false,

  createInstance(type: string, props: Props): TurInstance {
    const create = creators[type];
    if (!create) throw new Error(`unknown element type: ${type}`);
    const handle = create();
    setProps(handle, props);
    return { handle, type, props };
  },

  createTextInstance(): never {
    throw new Error("text instances not supported; use <tur_text_span>");
  },

  appendInitialChild(parentInstance: TurInstance, child: TurInstance): void {
    __tur.appendChild(ctx, parentInstance.handle, child.handle);
  },

  finalizeInitialChildren(): boolean {
    return false;
  },

  commitUpdate(
    instance: TurInstance,
    _type: any,
    _prevProps: Props,
    nextProps: Props,
    _internalHandle: any,
  ): void {
    setProps(instance.handle, nextProps);
    instance.props = nextProps;
  },

  appendChild(parentInstance: TurInstance, child: TurInstance): void {
    __tur.appendChild(ctx, parentInstance.handle, child.handle);
  },

  appendChildToContainer(container: TurNodeHandle, child: TurInstance): void {
    __tur.appendChild(ctx, container, child.handle);
  },

  insertBefore(parentInstance: TurInstance, child: TurInstance, before: TurInstance): void {
    __tur.insertBefore(ctx, parentInstance.handle, child.handle, before.handle);
  },

  insertInContainerBefore(container: TurNodeHandle, child: TurInstance, before: TurInstance): void {
    __tur.insertBefore(ctx, container, child.handle, before.handle);
  },

  removeChild(parentInstance: TurInstance, child: TurInstance): void {
    __tur.removeChild(ctx, parentInstance.handle, child.handle);
  },

  removeChildFromContainer(container: TurNodeHandle, child: TurInstance): void {
    __tur.removeChild(ctx, container, child.handle);
  },

  getRootHostContext(): null {
    return null;
  },

  getChildHostContext(): null {
    return null as any;
  },

  shouldSetTextContent(): boolean {
    return false;
  },

  getPublicInstance(instance: TurInstance): any {
    return instance.handle;
  },

  prepareForCommit(): null {
    return null;
  },

  resetAfterCommit(): void {},

  clearContainer(): void {},

  scheduleTimeout(fn: (...args: unknown[]) => unknown, delay?: number): unknown {
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
      : undefined as any,
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
  HostTransitionContext: null as any,
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
} as any);

let _container: any = null;

export function renderRoot(component: React.ComponentType): TurNodeHandle {
  const root = __tur.createRoot(ctx);
  _container = reconciler.createContainer(
    root,
    0,
    null,
    false,
    null,
    "",
    () => {},
    () => {},
    () => {},
    () => {},
  );

  const element = React.createElement(component);
  (reconciler as any).flushSyncFromReconciler(() => {
    reconciler.updateContainer(element, _container, null, () => {});
  });
  return root;
}
