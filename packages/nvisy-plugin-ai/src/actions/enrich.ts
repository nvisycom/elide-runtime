/**
 * AI-powered enrich action that extracts metadata, entities,
 * descriptions, or HTML from documents.
 *
 * @module
 */

import type { Metadata } from "@nvisy/core";
import { Action, Document } from "@nvisy/core";
import { z } from "zod";
import { AICompletionClient } from "../providers/client.js";
import { enrichByDescription } from "./enrich-by-description.js";
import { enrichByMetadata } from "./enrich-by-metadata.js";
import { enrichByNer } from "./enrich-by-ner.js";
import { enrichByTableHtml } from "./enrich-by-table-html.js";

export type { DescriptionEnrichParams } from "./enrich-by-description.js";
export type { MetadataEnrichParams } from "./enrich-by-metadata.js";
export type { NerEnrichParams } from "./enrich-by-ner.js";
export type { TableHtmlEnrichParams } from "./enrich-by-table-html.js";

const BaseMetadata = z.object({
	fields: z.array(z.string()),
});

const BaseNer = z.object({
	entityTypes: z.array(z.string()).optional(),
});

const BaseImageDescription = z.object({});

const BaseTableDescription = z.object({});

const BaseTableToHtml = z.object({});

const MetadataType = BaseMetadata.extend({
	type: z.literal("metadata"),
});

const NerType = BaseNer.extend({
	type: z.literal("ner"),
});

const ImageDescriptionType = BaseImageDescription.extend({
	type: z.literal("image_description"),
});

const TableDescriptionType = BaseTableDescription.extend({
	type: z.literal("table_description"),
});

const TableToHtmlType = BaseTableToHtml.extend({
	type: z.literal("table_to_html"),
});

const EnrichParams = z.discriminatedUnion("type", [
	MetadataType,
	NerType,
	ImageDescriptionType,
	TableDescriptionType,
	TableToHtmlType,
]);

/**
 * Enrich documents with AI-extracted metadata.
 *
 * - `"metadata"`: extract structured fields from content
 * - `"ner"`: named entity recognition
 * - `"image_description"`: describe image content
 * - `"table_description"`: describe table content
 * - `"table_to_html"`: convert table content to HTML
 */
export const enrich = Action.withClient("enrich", AICompletionClient, {
	types: [Document],
	params: EnrichParams,
	transform: transformEnrich,
});

async function* transformEnrich(
	stream: AsyncIterable<Document>,
	params: z.infer<typeof EnrichParams>,
	client: AICompletionClient,
): AsyncGenerator<Document> {
	for await (const doc of stream) {
		let enrichedMeta: Metadata;

		switch (params.type) {
			case "metadata": {
				const { type: _, ...rest } = params;
				enrichedMeta = await enrichByMetadata(doc.content, rest, client);
				break;
			}
			case "ner": {
				const { type: _, ...rest } = params;
				enrichedMeta = await enrichByNer(doc.content, rest, client);
				break;
			}
			case "image_description": {
				const { type: _, ...rest } = params;
				enrichedMeta = await enrichByDescription(
					doc.content,
					{ ...rest, contentKind: "image" },
					client,
				);
				break;
			}
			case "table_description": {
				const { type: _, ...rest } = params;
				enrichedMeta = await enrichByDescription(
					doc.content,
					{ ...rest, contentKind: "table" },
					client,
				);
				break;
			}
			case "table_to_html": {
				const { type: _, ...rest } = params;
				enrichedMeta = await enrichByTableHtml(doc.content, rest, client);
				break;
			}
		}

		yield new Document(doc.content, {
			...(doc.elements != null ? { elements: doc.elements } : {}),
		})
			.deriveFrom(doc)
			.withMetadata({ ...(doc.metadata ?? {}), ...enrichedMeta });
	}
}
