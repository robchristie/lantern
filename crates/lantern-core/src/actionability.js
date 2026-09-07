function(selector, action, previous) {
    const matches = document.querySelectorAll(selector);
    if (matches.length > 1) return {error: 'ambiguous_selector'};
    if (matches.length === 0) return {error: 'selector_not_found'};
    if (matches[0] !== this || !this.isConnected) return {error: 'element_unstable'};
    const globalKey = action === 'key' && this === document.body;
    const requiresEnabled = ['click', 'type', 'key'].includes(action);
    if (!globalKey && requiresEnabled &&
        (this.matches(':disabled') || this.closest('[aria-disabled="true" i], [inert]')))
        return {error: 'element_disabled', node_name: this.nodeName.slice(0, 64)};
    const editable = () => {
        const textInput = this instanceof HTMLInputElement &&
            ['text', 'search', 'email', 'url', 'tel', 'password', 'number'].includes(this.type);
        return (textInput || this instanceof HTMLTextAreaElement || this.isContentEditable) &&
            !this.readOnly && !this.closest('[aria-readonly="true" i]');
    };
    if (action === 'type' && !editable())
        return {error: 'element_not_editable', node_name: this.nodeName.slice(0, 64)};

    // body is the explicit global-key route: preserve the current focus.
    if (globalKey) return {node_name: this.nodeName.slice(0, 64), rect: [0, 0, 0, 0]};
    if (!previous) {
        this.scrollIntoView({block: 'center', inline: 'center', behavior: 'instant'});
        if (action === 'type' || action === 'key') this.focus({preventScroll: true});
    }
    if ((action === 'type' || action === 'key') && document.activeElement !== this)
        return {error: 'element_not_focused', node_name: this.nodeName.slice(0, 64)};
    // Focus/scroll listeners may synchronously replace or disable the target.
    if (!this.isConnected || document.querySelectorAll(selector).length !== 1 ||
        document.querySelector(selector) !== this) return {error: 'element_unstable'};
    if (requiresEnabled && (this.matches(':disabled') || this.closest('[aria-disabled="true" i], [inert]')))
        return {error: 'element_disabled', node_name: this.nodeName.slice(0, 64)};
    if (action === 'type' && !editable())
        return {error: 'element_not_editable', node_name: this.nodeName.slice(0, 64)};
    const box = this.getBoundingClientRect();
    const rect = [box.x, box.y, box.width, box.height];
    const style = getComputedStyle(this);
    if (style.visibility !== 'visible' || box.width <= 0 || box.height <= 0)
        return {error: 'element_not_visible', node_name: this.nodeName.slice(0, 64)};
    if (previous && rect.some((value, index) => Math.abs(value - previous[index]) > 0.01))
        return {error: 'element_unstable', node_name: this.nodeName.slice(0, 64)};
    // Use the visible border box: padded zero-content controls still receive input.
    const left = Math.max(0, box.left), right = Math.min(innerWidth, box.right);
    const top = Math.max(0, box.top), bottom = Math.min(innerHeight, box.bottom);
    if (left >= right || top >= bottom) return {error: 'element_not_visible', node_name: this.nodeName.slice(0, 64)};
    const point = {x: (left + right) / 2, y: (top + bottom) / 2};
    const hit = document.elementFromPoint(point.x, point.y);
    if (!hit || !(hit === this || this.contains(hit)))
        return {error: 'element_occluded', node_name: this.nodeName.slice(0, 64)};
    return {node_name: this.nodeName.slice(0, 64), rect, point};
}
