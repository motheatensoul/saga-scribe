import { invoke } from '@tauri-apps/api/core';
import type { Template } from './stores/template';
import type { Entity, EntityMap } from './stores/entities';

export interface FileContent {
    path: string;
    content: string;
}

export async function openFile(path: string): Promise<FileContent> {
    return invoke('open_file', { path });
}

export async function saveFile(path: string, content: string): Promise<void> {
    return invoke('save_file', { path, content });
}

export async function exportTei(path: string, teiContent: string): Promise<void> {
    return invoke('export_tei', { path, teiContent });
}

export async function listTemplates(): Promise<Template[]> {
    return invoke('list_templates');
}

export async function getTemplate(id: string): Promise<Template> {
    return invoke('get_template', { id });
}

export async function saveTemplate(template: Template): Promise<void> {
    return invoke('save_template', { template });
}

export interface CompileOptions {
    wordWrap?: boolean;
    autoLineNumbers?: boolean;
    entitiesJson?: string;
}

export async function compileDsl(
    input: string,
    templateHeader: string,
    templateFooter: string,
    options?: CompileOptions
): Promise<string> {
    return invoke('compile_dsl', {
        input,
        templateHeader,
        templateFooter,
        wordWrap: options?.wordWrap ?? false,
        autoLineNumbers: options?.autoLineNumbers ?? false,
        entitiesJson: options?.entitiesJson ?? null,
    });
}

// Entity functions
export async function loadEntities(path: string): Promise<EntityMap> {
    return invoke('load_entities', { path });
}

export async function getEntity(path: string, name: string): Promise<Entity | null> {
    return invoke('get_entity', { path, name });
}

export async function listEntityNames(path: string): Promise<string[]> {
    return invoke('list_entity_names', { path });
}
