#!/usr/bin/env python

import serial
import argparse

parser = argparse.ArgumentParser()
parser.add_argument('-d', '--device', help='Serial device to use')
parser.add_argument('-b', '--baudrate', type=int, default=2500000, help='Baudrate to use')
args = parser.parse_args()

device_name = args.device.split("/")[-1]
procfile = "/sys/bus/usb-serial/devices/{0}/latency_timer".format(device_name)

with open(procfile, 'w') as p:
    p.write("1")
    p.flush()

ser = serial.Serial(args.device, args.baudrate)
ser.send_break()
ser.write(b"UUUUUUUU")
