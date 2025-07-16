#!/bin/bash

echo "Testing manual gate in zim-dsp"
echo "=============================="
echo
echo "Step 1: Load the manual gate patch"
echo "Step 2: Start audio"
echo "Step 3: Press 'g' to activate gate (play note)"
echo "Step 4: Press 'r' to release gate (stop note)"
echo
echo "Commands to copy/paste:"
echo
echo "First, paste all these lines to set up the patch:"
echo "gate: manual"
echo "vco: osc sine 440"
echo "env: envelope 0.01 0.5"
echo "vca: vca 1.0"
echo "env.gate <- gate.gate"
echo "vca.audio <- vco.sine"
echo "vca.cv <- env.out"
echo "out <- vca.out"
echo
echo "Then:"
echo "start    # Start audio"
echo "g        # Press gate (you should hear a note)"
echo "r        # Release gate (note should stop)"
echo "g        # Press again"
echo "r        # Release again"
echo "stop     # Stop audio"
echo "quit     # Exit"