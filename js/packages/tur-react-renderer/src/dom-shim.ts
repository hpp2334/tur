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
  tur_root: () => __tur.createRoot(ctx),
};

function indexOfChild(parent: TurNode, child: TurNode): number {
  for (let i = 0; i < parent._children.length; i++) {
    if (parent._children[i] === child) return i;
  }
  return -1;
}

export class TurNode {
  _handle: TurNodeHandle;
  _tag: string;
  _parent: TurNode | null = null;
  _children: TurNode[] = [];
  _props: Record<string, unknown> = {};

  constructor(tag: string, handle: TurNodeHandle) {
    this._tag = tag;
    this._handle = handle;
  }

  get nodeType(): number {
    return 1;
  }

  get parentNode(): TurNode | null {
    return this._parent;
  }

  get firstChild(): TurNode | null {
    return this._children[0] ?? null;
  }

  get nextSibling(): TurNode | null {
    if (!this._parent) return null;
    const siblings = this._parent._children;
    const idx = indexOfChild(this._parent, this);
    if (idx < 0 || idx + 1 >= siblings.length) return null;
    return siblings[idx + 1];
  }

  get childNodes(): TurNode[] {
    return this._children;
  }

  appendChild(child: TurNode): TurNode {
    if (child._parent) {
      child._parent.removeChild(child);
    }
    __tur.appendChild(ctx, this._handle, child._handle);
    child._parent = this;
    this._children.push(child);
    return child;
  }

  removeChild(child: TurNode): TurNode {
    const idx = indexOfChild(this, child);
    if (idx >= 0) {
      __tur.removeChild(ctx, this._handle, child._handle);
      this._children.splice(idx, 1);
      child._parent = null;
    }
    return child;
  }

  insertBefore(newChild: TurNode, refChild: TurNode | null): TurNode {
    if (newChild._parent) {
      newChild._parent.removeChild(newChild);
    }
    if (refChild === null) {
      return this.appendChild(newChild);
    }
    const idx = indexOfChild(this, refChild);
    if (idx < 0) {
      return this.appendChild(newChild);
    }
    __tur.insertBefore(ctx, this._handle, newChild._handle, refChild._handle);
    newChild._parent = this;
    this._children.splice(idx, 0, newChild);
    return newChild;
  }

  setAttribute(name: string, value: unknown): void {
    if (name === "ref") return;
    if (name === "children") return;
    if (value === null || value === undefined) {
      delete this._props[name];
      return;
    }
    this._props[name] = value;
    __tur.setAttribute(ctx, this._handle, name, value);
  }

  removeAttribute(name: string): void {
    delete this._props[name];
  }

  addEventListener(): void {}

  removeEventListener(): void {}
}

export class TurText {
  _data: string;
  _parent: TurNode | null = null;

  constructor(data: string) {
    this._data = data;
  }

  get nodeType(): number {
    return 3;
  }

  get parentNode(): TurNode | null {
    return this._parent;
  }

  get data(): string {
    return this._data;
  }

  set data(value: string) {
    this._data = value;
  }

  get nextSibling(): TurNode | null {
    if (!this._parent) return null;
    const siblings = this._parent._children;
    const idx = siblings.indexOf(this as unknown as TurNode);
    if (idx < 0 || idx + 1 >= siblings.length) return null;
    return siblings[idx + 1];
  }
}

export class TurDocument {
  createElement(tag: string): TurNode {
    const create = creators[tag];
    if (!create) throw new Error(`unknown element type: ${tag}`);
    return new TurNode(tag, create());
  }

  createTextNode(data: string): TurText {
    return new TurText(data);
  }
}
