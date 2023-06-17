SHELL := /bin/bash
CORES=56
FIBER_MULTIPLE=112
MANYVARS=17000000
BINPREFIX=target/release/trust_eval

target/release/mutex_eval:
	cargo build --release --bin lock_eval
	mv target/release/lock_eval target/release/mutex_eval
target/release/spinlock_eval:
	cargo build --release --features=spinlock --bin lock_eval
	mv target/release/lock_eval target/release/spinlock_eval


target/release/trust_eval_default:
	cargo build --release
	mv target/release/trust_eval target/release/trust_eval_default

# standard throughput experiment

eval/default_tput.txt: target/release/trust_eval_default
	for ((v=1;v<$(CORES);v+=4)); do $< -k $(CORES) -v $$v -f $(FIBER_MULTIPLE); done > $@
eval/async_tput.txt: target/release/trust_eval_default
	for ((v=1;v<$(CORES);v+=4)); do $< -k $(CORES) -v $$v -x async_return; done > $@

eval/mutex_tput.txt: target/release/mutex_eval
	for ((v=9;v<$(CORES);v+=4)); do $< -k $(CORES) -v $$v; done > $@
eval/spinlock_tput.txt: target/release/spinlock_eval
	for ((v=9;v<$(CORES);v+=4)); do $< -k $(CORES) -v $$v; done > $@

eval/default_tput_manyvars.txt: target/release/trust_eval_default
	for ((v=1;v<=$(MANYVARS);v*=2)); do $<  -k $(CORES) -v $$v -f $(FIBER_MULTIPLE); done > $@
eval/async_tput_manyvars.txt: target/release/trust_eval_default
	for ((v=1;v<=$(MANYVARS);v*=2)); do $<  -k $(CORES) -v $$v -f 1 -x async_return; done > $@
eval/default_tput_manyvars_onefiber.txt: target/release/trust_eval_default
	for ((v=1;v<=$(MANYVARS);v*=16)); do $<  -k $(CORES) -v $$v -f 1; done > $@

eval/mutex_tput_manyvars.txt: target/release/mutex_eval
	for ((v=16;v<=$(MANYVARS);v*=2)); do $< -k $(CORES) -v $$v; done > $@
eval/spinlock_tput_manyvars.txt: target/release/spinlock_eval
	for ((v=16;v<=$(MANYVARS);v*=2)); do $< -k $(CORES) -v $$v; done > $@
	
eval/default_tput_fibers.txt: target/release/trust_eval_default
	for ((f=1;f<=$(FIBER_MULTIPLE);f*=2)); do $< -k $(CORES) -v 56 -f $$f; done > $@

eval/throughput.pdf: eval/throughput.gpl eval/default_tput.txt eval/async_tput.txt eval/spinlock_tput.txt eval/mutex_tput.txt
	(pushd eval;\
	gnuplot < throughput.gpl;\
	popd;)

eval/throughput_manyvars.pdf: eval/throughput_manyvars.gpl eval/async_tput_manyvars.txt eval/default_tput_manyvars.txt eval/default_tput_manyvars_onefiber.txt  eval/spinlock_tput_manyvars.txt eval/mutex_tput_manyvars.txt 
	(pushd eval;\
	gnuplot < throughput_manyvars.gpl;\
	popd;)

eval/throughput_fibers.pdf: eval/default_tput_fibers.txt eval/throughput_fibers.gpl
	(pushd eval;\
	gnuplot < throughput_fibers.gpl;\
	popd;)

plots: eval/throughput_manyvars.pdf eval/throughput.pdf

clean: 
	rm -f eval/*.txt eval/*.pdf
	rm -f target/release/*_eval*
