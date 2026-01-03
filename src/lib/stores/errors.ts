import { writable, derived } from 'svelte/store';

export interface AppError {
    id: string;
    timestamp: Date;
    level: 'error' | 'warning' | 'info';
    source: string;
    message: string;
    details?: string;
}

function createErrorStore() {
    const { subscribe, update, set } = writable<AppError[]>([]);

    let idCounter = 0;

    return {
        subscribe,
        add: (error: Omit<AppError, 'id' | 'timestamp'>) => {
            const newError: AppError = {
                ...error,
                id: `error-${++idCounter}`,
                timestamp: new Date(),
            };
            console.log(`[${error.level.toUpperCase()}] ${error.source}: ${error.message}`, error.details || '');
            update((errors) => [...errors, newError].slice(-50)); // Keep last 50 errors
            return newError.id;
        },
        error: (source: string, message: string, details?: string) => {
            console.error(`[ERROR] ${source}: ${message}`, details || '');
            update((errors) => [
                ...errors,
                {
                    id: `error-${++idCounter}`,
                    timestamp: new Date(),
                    level: 'error' as const,
                    source,
                    message,
                    details,
                },
            ].slice(-50));
        },
        warning: (source: string, message: string, details?: string) => {
            console.warn(`[WARNING] ${source}: ${message}`, details || '');
            update((errors) => [
                ...errors,
                {
                    id: `error-${++idCounter}`,
                    timestamp: new Date(),
                    level: 'warning' as const,
                    source,
                    message,
                    details,
                },
            ].slice(-50));
        },
        info: (source: string, message: string, details?: string) => {
            console.info(`[INFO] ${source}: ${message}`, details || '');
            update((errors) => [
                ...errors,
                {
                    id: `error-${++idCounter}`,
                    timestamp: new Date(),
                    level: 'info' as const,
                    source,
                    message,
                    details,
                },
            ].slice(-50));
        },
        remove: (id: string) => update((errors) => errors.filter((e) => e.id !== id)),
        clear: () => set([]),
    };
}

export const errorStore = createErrorStore();

// Derived store for error count by level
export const errorCounts = derived(errorStore, ($errors) => ({
    error: $errors.filter((e) => e.level === 'error').length,
    warning: $errors.filter((e) => e.level === 'warning').length,
    info: $errors.filter((e) => e.level === 'info').length,
    total: $errors.length,
}));
