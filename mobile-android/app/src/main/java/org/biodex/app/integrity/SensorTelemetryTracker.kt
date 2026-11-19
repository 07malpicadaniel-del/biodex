package org.biodex.app.integrity

import android.annotation.SuppressLint
import android.content.Context
import android.hardware.Sensor
import android.hardware.SensorEvent
import android.hardware.SensorEventListener
import android.hardware.SensorManager
import android.location.Location
import android.location.LocationListener
import android.location.LocationManager
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update

data class TelemetrySnapshot(
    val accelXyz: FloatArray = floatArrayOf(0f, 0f, 9.81f),
    val gyroXyz: FloatArray = floatArrayOf(0f, 0f, 0f),
    val hasImuFix: Boolean = false,
    val latitude: Double? = null,
    val longitude: Double? = null,
    val accuracyMeters: Float = 12.0f,
) {
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is TelemetrySnapshot) return false
        return accelXyz.contentEquals(other.accelXyz) &&
            gyroXyz.contentEquals(other.gyroXyz) &&
            hasImuFix == other.hasImuFix &&
            latitude == other.latitude &&
            longitude == other.longitude &&
            accuracyMeters == other.accuracyMeters
    }

    override fun hashCode(): Int {
        var result = accelXyz.contentHashCode()
        result = 31 * result + gyroXyz.contentHashCode()
        result = 31 * result + hasImuFix.hashCode()
        result = 31 * result + (latitude?.hashCode() ?: 0)
        result = 31 * result + (longitude?.hashCode() ?: 0)
        result = 31 * result + accuracyMeters.hashCode()
        return result
    }
}

class SensorTelemetryTracker(context: Context) {
    private val sensorManager = context.getSystemService(Context.SENSOR_SERVICE) as SensorManager
    private val locationManager = context.getSystemService(Context.LOCATION_SERVICE) as LocationManager

    private val _snapshot = MutableStateFlow(TelemetrySnapshot())
    val snapshot: StateFlow<TelemetrySnapshot> = _snapshot.asStateFlow()

    private val sensorListener = object : SensorEventListener {
        override fun onSensorChanged(event: SensorEvent) {
            when (event.sensor.type) {
                Sensor.TYPE_ACCELEROMETER -> {
                    val copy = floatArrayOf(event.values[0], event.values[1], event.values[2])
                    _snapshot.update { it.copy(accelXyz = copy, hasImuFix = true) }
                }
                Sensor.TYPE_GYROSCOPE -> {
                    val copy = floatArrayOf(event.values[0], event.values[1], event.values[2])
                    _snapshot.update { it.copy(gyroXyz = copy, hasImuFix = true) }
                }
            }
        }

        override fun onAccuracyChanged(sensor: Sensor?, accuracy: Int) = Unit
    }

    private val locationListener = LocationListener { location ->
        updateLocation(location)
    }

    fun startImu() {
        sensorManager.getDefaultSensor(Sensor.TYPE_ACCELEROMETER)?.let { sensor ->
            sensorManager.registerListener(sensorListener, sensor, SensorManager.SENSOR_DELAY_UI)
        }
        sensorManager.getDefaultSensor(Sensor.TYPE_GYROSCOPE)?.let { sensor ->
            sensorManager.registerListener(sensorListener, sensor, SensorManager.SENSOR_DELAY_UI)
        }
    }

    @SuppressLint("MissingPermission")
    fun startLocationUpdates() {
        val providers = listOf(LocationManager.GPS_PROVIDER, LocationManager.NETWORK_PROVIDER)
        for (provider in providers) {
            runCatching {
                if (locationManager.isProviderEnabled(provider)) {
                    locationManager.getLastKnownLocation(provider)?.let(::updateLocation)
                    locationManager.requestLocationUpdates(provider, 2_000L, 1.0f, locationListener)
                }
            }
        }
    }

    fun stop() {
        sensorManager.unregisterListener(sensorListener)
        runCatching { locationManager.removeUpdates(locationListener) }
    }

    private fun updateLocation(location: Location) {
        _snapshot.update {
            it.copy(
                latitude = location.latitude,
                longitude = location.longitude,
                accuracyMeters = if (location.hasAccuracy()) location.accuracy else 15.0f,
            )
        }
    }
}
