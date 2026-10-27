package dev.darksavci.spotified.data

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import kotlinx.coroutines.channels.awaitClose
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.callbackFlow
import kotlinx.coroutines.flow.distinctUntilChanged

/** Whether the phone is on a network that costs: mobile data, a metered hotspot. */
fun meteredNetwork(context: Context): Flow<Boolean> = callbackFlow {
    val cm = context.getSystemService(ConnectivityManager::class.java)
    fun metered() = cm.activeNetwork?.let { cm.getNetworkCapabilities(it) }
        ?.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED) != true
    trySend(metered())
    val callback = object : ConnectivityManager.NetworkCallback() {
        override fun onCapabilitiesChanged(network: Network, caps: NetworkCapabilities) {
            trySend(metered())
        }
        override fun onLost(network: Network) {
            trySend(metered())
        }
    }
    cm.registerDefaultNetworkCallback(callback)
    awaitClose { cm.unregisterNetworkCallback(callback) }
}.distinctUntilChanged()
