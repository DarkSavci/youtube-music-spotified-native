package dev.darksavci.spotified.api

import kotlinx.serialization.KSerializer
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.builtins.nullable
import kotlinx.serialization.descriptors.SerialDescriptor
import kotlinx.serialization.encoding.Decoder
import kotlinx.serialization.encoding.Encoder

/*
 * Items travel as {"kind": ..., "<kind>": {...}}. They are read through
 * WireItem and turned into Item, dropping kinds this build does not know.
 * They are never sent back, so writing them is not supported.
 */

object ItemSerializer : KSerializer<Item?> {
    private val wire = WireItem.serializer().nullable
    override val descriptor: SerialDescriptor = wire.descriptor

    override fun deserialize(decoder: Decoder): Item? = decoder.decodeSerializableValue(wire)?.item()

    override fun serialize(encoder: Encoder, value: Item?) =
        throw UnsupportedOperationException("items are only read")
}

object ItemListSerializer : KSerializer<List<Item>> {
    private val wire = ListSerializer(WireItem.serializer()).nullable
    override val descriptor: SerialDescriptor = wire.descriptor

    override fun deserialize(decoder: Decoder): List<Item> =
        decoder.decodeSerializableValue(wire).orEmpty().mapNotNull { it.item() }

    override fun serialize(encoder: Encoder, value: List<Item>) =
        throw UnsupportedOperationException("items are only read")
}
