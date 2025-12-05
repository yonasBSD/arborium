class EventEmitter {
    #listeners = new Map();

    on(event, callback) {
        if (!this.#listeners.has(event)) {
            this.#listeners.set(event, []);
        }
        this.#listeners.get(event).push(callback);
    }

    emit(event, ...args) {
        const callbacks = this.#listeners.get(event) ?? [];
        callbacks.forEach(cb => cb(...args));
    }
}

async function fetchData(url) {
    const response = await fetch(url);
    const { items } = await response.json();
    return items.map(item => item.name);
}
