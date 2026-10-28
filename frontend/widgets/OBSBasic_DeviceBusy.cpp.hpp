/******************************************************************************
    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#include "frontend/build/frontend-widgets.pch.hpp"

/* A capture source found its device held by another process (EBUSY), and
 * stays dead until that process lets go (obs-rust/obs-studio#799). The source
 * announces it once per busy episode; the popup names the holder and offers
 * Force Camera, which calls the source's force_camera proc: SIGTERM, never
 * SIGKILL, and only for a process of the same user. */

const char *OBSBasic::sourceDeviceBusyDecl = "void source_device_busy(ptr source, string device, string holder, "
					     "int pid, bool is_obs, bool can_force)";

/* Called on the thread that opened the device */
void OBSBasic::SourceDeviceBusy(void *data, calldata_t *params)
{
	OBSBasic *main = static_cast<OBSBasic *>(data);
	obs_source_t *source = static_cast<obs_source_t *>(calldata_ptr(params, "source"));
	if (!source) {
		return;
	}

	OBSWeakSource weak = OBSGetWeakRef(source);
	QString device = QT_UTF8(calldata_string(params, "device"));
	QString holder = QT_UTF8(calldata_string(params, "holder"));
	qint64 pid = calldata_int(params, "pid");
	bool isObs = calldata_bool(params, "is_obs");
	bool canForce = calldata_bool(params, "can_force");

	QMetaObject::invokeMethod(
		main, [=]() { main->ShowDeviceBusy(weak, device, holder, pid, isObs, canForce); },
		Qt::QueuedConnection);
}

void OBSBasic::ShowDeviceBusy(OBSWeakSource weak, QString device, QString holder, qint64 pid, bool isObs, bool canForce)
{
	/* one popup per device; the source retries on its own meanwhile */
	if (busyDevicePopups.contains(device)) {
		return;
	}
	OBSSource source = OBSGetStrongRef(weak);
	if (!source) {
		return;
	}

	QString name = QT_UTF8(obs_source_get_name(source));
	QString text = holder.isEmpty()
			       ? QTStr("Basic.DeviceBusy.Unknown").arg(name, device)
			       : QTStr("Basic.DeviceBusy.Text").arg(name, device, holder, QString::number(pid));
	if (isObs) {
		text += "\n\n" + QTStr("Basic.DeviceBusy.OtherOBS");
	}
	text += "\n\n" +
		(canForce ? QTStr("Basic.DeviceBusy.ForceExplain").arg(holder) : QTStr("Basic.DeviceBusy.Wait"));
	source = nullptr;

	QMessageBox messageBox(this);
	messageBox.setIcon(QMessageBox::Warning);
	messageBox.setWindowTitle(QTStr("Basic.DeviceBusy.Title"));
	messageBox.setText(text);
	QPushButton *force = canForce ? messageBox.addButton(QTStr("Basic.DeviceBusy.Force"), QMessageBox::AcceptRole)
				      : nullptr;
	messageBox.addButton(canForce ? QMessageBox::Cancel : QMessageBox::Ok);
	if (force) {
		messageBox.setDefaultButton(force);
	}

	busyDevicePopups.insert(device);
	messageBox.exec();
	busyDevicePopups.remove(device);

	if (!force || messageBox.clickedButton() != force) {
		return;
	}

	source = OBSGetStrongRef(weak);
	if (!source) {
		return;
	}

	calldata_t cd = {};
	bool released = proc_handler_call(obs_source_get_proc_handler(source), "force_camera", &cd) &&
			calldata_bool(&cd, "released");
	calldata_free(&cd);

	if (!released) {
		OBSMessageBox::warning(this, QTStr("Basic.DeviceBusy.Title"),
				       QTStr("Basic.DeviceBusy.ForceFailed").arg(holder, QString::number(pid)));
	}
}
