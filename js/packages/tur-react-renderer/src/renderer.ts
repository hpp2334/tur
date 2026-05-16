import ReactReconciler from "react-reconciler";
import type { ReactElement } from "react";
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

let updatePriority = 1;

const reconciler = ReactReconciler<
  string,
  Props,
  TurNodeHandle,
  TurInstance,
  never,
  never,
  never,
  never,
  TurInstance,
  null,
  never,
  unknown,
  undefined,
  undefined
>({
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
    _updatePayload: null,
    _type: string,
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
    return instance;
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
  getCurrentUpdatePriority: () => updatePriority,
  setCurrentUpdatePriority(p: number): void {
    updatePriority = p;
  },
  resolveUpdatePriority(): number {
    return updatePriority;
  },
  shouldAttemptEagerTransition(): boolean {
    return false;
  },
  maySuspendCommit(): boolean {
    return false;
  },
  preloadInstance(): void {},
  startSuspendingCommit(): void {},
  suspendInstance(): never {
    throw new Error("suspendInstance not supported");
  },
  waitForCommitToBeReady(): never {
    throw new Error("waitForCommitToBeReady not supported");
  },
  NotPendingTransition: null,
  HostTransitionContext: null,
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
});

export function renderRoot(component: () => ReactElement): TurNodeHandle {
  const root = __tur.createRoot(ctx);
  const container = reconciler.createContainer(
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
  (reconciler as any).flushSyncFromReconciler(() => {
    reconciler.updateContainer(component(), container, null, () => {});
  });
  return root;
}

export { reconciler };
