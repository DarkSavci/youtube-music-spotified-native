package dev.darksavci.spotified.core

import android.content.Context
import java.io.File
import java.util.UUID

/**
 * The accounts signed in on this phone, each with its own folder: its
 * credentials, its databases, its history. The core runs on one at a time;
 * switching starts it again on another. The first account lives in the
 * app's own folder, where a single sign-in has always been kept.
 */
class Accounts(context: Context) {
    private val root = context.filesDir
    private val chosen = File(root, "account.txt")
    private val names = File(root, "accounts.txt")

    data class Entry(val id: String, val name: String)

    /** The account in use, by id; "" is the first. */
    var current: String
        get() = chosen.takeIf { it.exists() }?.readText()?.trim()?.takeIf { dirOf(it).exists() }.orEmpty()
        set(value) = chosen.writeText(value)

    fun dirOf(id: String): File = if (id.isEmpty()) root else File(root, "accounts/$id").apply { mkdirs() }

    /** The folder the core keeps the account in use in. */
    fun currentDir(): File = dirOf(current)

    fun list(): List<Entry> {
        val known = names.takeIf { it.exists() }?.readLines().orEmpty()
            .mapNotNull { line -> line.split('\t').takeIf { it.size == 2 }?.let { Entry(it[0], it[1]) } }
        val first = Entry("", known.firstOrNull { it.id.isEmpty() }?.name ?: "First account")
        val others = File(root, "accounts").listFiles().orEmpty().filter { it.isDirectory }.map { dir ->
            known.firstOrNull { it.id == dir.name } ?: Entry(dir.name, "Another account")
        }
        return listOf(first) + others
    }

    fun signedIn(id: String) = File(dirOf(id), "credentials.json").exists()

    /** Remembers the name an account shows as, once it is known. */
    fun name(id: String, name: String) {
        val rest = list().filter { it.id != id }
        names.writeText((rest + Entry(id, name)).joinToString("") { "${it.id}\t${it.name}\n" })
    }

    /** A new, empty account folder, made the one in use. */
    fun add(): String {
        val id = UUID.randomUUID().toString().take(8)
        dirOf(id)
        current = id
        return id
    }

    /** Forgets an account and everything kept for it; the first account is signed out instead. */
    fun remove(id: String) {
        if (id.isEmpty()) {
            File(root, "credentials.json").delete()
            return
        }
        dirOf(id).deleteRecursively()
        if (current == id) current = ""
    }
}
