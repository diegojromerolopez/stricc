void signal(int sig, void *handler);
unsigned int alarm(unsigned int seconds);
void exit(int status);

void handle_alarm(int sig) {
    exit(0);
}

int main() {
    signal(14, (void*)handle_alarm); // SIGALRM is 14
    alarm(1);
    while (1) {
        // Infinite loop with no side effects
    }
    return 1;
}
