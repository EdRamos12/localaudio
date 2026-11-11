package net.localaudio.player

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.net.wifi.WifiManager
import android.os.Build
import android.util.Log
import java.net.Inet4Address
import java.util.concurrent.Executors

data class FoundServer(val name: String, val host: String, val tcpPort: Int)

class Discovery(
    context: Context,
    private val onServer: (FoundServer) -> Unit,
    private val onLog: (String) -> Unit,
) {
    private val nsd = context.getSystemService(NsdManager::class.java)
    private val lock = (context.applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager)
        .createMulticastLock("localaudio")
        .apply { setReferenceCounted(false) }
    private val executor = Executors.newSingleThreadExecutor()
    private var listening = false

    private val listener = object : NsdManager.DiscoveryListener {
        override fun onStartDiscoveryFailed(serviceType: String, errorCode: Int) {
            onLog("Discovery failed to start ($errorCode)")
        }

        override fun onStopDiscoveryFailed(serviceType: String, errorCode: Int) = Unit

        override fun onDiscoveryStarted(serviceType: String) {
            onLog("Looking for PCs")
        }

        override fun onDiscoveryStopped(serviceType: String) = Unit

        override fun onServiceFound(serviceInfo: NsdServiceInfo) {
            resolve(serviceInfo)
        }

        override fun onServiceLost(serviceInfo: NsdServiceInfo) = Unit
    }

    fun start() {
        if (listening) return
        lock.acquire()
        nsd.discoverServices(SERVICE_TYPE, NsdManager.PROTOCOL_DNS_SD, listener)
        listening = true
    }

    fun stop() {
        if (!listening) return
        runCatching { nsd.stopServiceDiscovery(listener) }
        if (lock.isHeld) lock.release()
        listening = false
        executor.shutdownNow()
    }

    private fun resolve(serviceInfo: NsdServiceInfo) {
        if (Build.VERSION.SDK_INT >= 34) {
            nsd.registerServiceInfoCallback(serviceInfo, executor, object : NsdManager.ServiceInfoCallback {
                override fun onServiceInfoCallbackRegistrationFailed(errorCode: Int) {
                    onLog("Could not resolve ${serviceInfo.serviceName} ($errorCode)")
                }

                override fun onServiceUpdated(updated: NsdServiceInfo) {
                    publish(updated)
                }

                override fun onServiceLost() = Unit

                override fun onServiceInfoCallbackUnregistered() = Unit
            })
        } else {
            @Suppress("DEPRECATION")
            nsd.resolveService(serviceInfo, object : NsdManager.ResolveListener {
                override fun onResolveFailed(serviceInfo: NsdServiceInfo, errorCode: Int) {
                    onLog("Could not resolve ${serviceInfo.serviceName} ($errorCode)")
                }

                override fun onServiceResolved(resolved: NsdServiceInfo) {
                    publish(resolved)
                }
            })
        }
    }

    private fun publish(info: NsdServiceInfo) {
        val host = hostAddress(info) ?: return
        val tcp = info.attributes["tcp"]?.let { bytes ->
            bytes.toString(Charsets.UTF_8).trim().trim('\u0000').toIntOrNull()
        } ?: return
        val name = info.attributes["name"]?.toString(Charsets.UTF_8)?.trim().orEmpty()
            .ifEmpty { info.serviceName }
        Log.i(TAG, "found $name at $host:$tcp")
        onServer(FoundServer(name, host, tcp))
    }

    private fun hostAddress(info: NsdServiceInfo): String? {
        if (Build.VERSION.SDK_INT >= 34) {
            val addresses = info.hostAddresses
            val picked = addresses.firstOrNull { it is Inet4Address } ?: addresses.firstOrNull()
            if (picked != null) return picked.hostAddress
        }
        @Suppress("DEPRECATION")
        return info.host?.hostAddress
    }

    companion object {
        private const val SERVICE_TYPE = "_localaudio._udp."
        const val TAG = "LocalAudio"
    }
}
