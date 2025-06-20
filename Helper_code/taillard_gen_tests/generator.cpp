#include <iostream>
#include <fstream>
#include <vector>
#include <random>
#include <string>
#include <iomanip>
#include <algorithm>
#include <climits>

int unif(std::mt19937 &rng, int low, int high) {
    std::uniform_int_distribution<int> dist(low, high);
    return dist(rng);
}

void generate_instance(int jobs, int machines, int seed_time, int seed_machine, std::string filename) {
    std::mt19937 time_rng(seed_time);
    std::mt19937 machine_rng(seed_machine);

    std::vector<std::vector<int>> durations(jobs, std::vector<int>(machines));
    std::vector<std::vector<int>> machine_order(jobs, std::vector<int>(machines));

    // Fill durations randomly between 1 and 99
    for (int j = 0; j < jobs; ++j)
        for (int m = 0; m < machines; ++m)
            durations[j][m] = unif(time_rng, 1, 99);

    // Assign machines in order then randomly permute for each job
    for (int j = 0; j < jobs; ++j) {
        for (int m = 0; m < machines; ++m)
            machine_order[j][m] = m + 1; // Machines indexed from 1 for MiniZinc

        shuffle(machine_order[j].begin(), machine_order[j].end(), machine_rng); // Shuffle machine order on each row (instead of std::swap(M[i][j], M[U(i,m)][j]))
    }

    // MiniZinc format
    std::ofstream out(filename);
    out << "n = " << jobs << ";\n";
    out << "m = " << machines << ";\n\n";

    out << "d = [| ";
    for (int j = 0; j < jobs; ++j) {
        for (int m = 0; m < machines; ++m)
            out << durations[j][m] << (m < machines - 1 ? ", " : "");
        out << (j < jobs - 1 ? " |\n     " : " |];\n\n");
    }

    out << "mc = [| ";
    for (int j = 0; j < jobs; ++j) {
        for (int m = 0; m < machines; ++m)
            out << machine_order[j][m] << (m < machines - 1 ? ", " : "");
        out << (j < jobs - 1 ? " |\n      " : " |];\n");
    }

    out.close();
    std::cout << "Generated: " << filename << "\n";
}

int main() {
    // Log file: record CSV (filename, time_seed, machine_seed)
    std::ofstream log("log.txt");
    log << "Seeds for which instances were generated:\n";
    log << "Filename, Time Seed, Machine Seed\n";

    std::vector<int> inst,job,mach;
    int instances, jobs, machines;

    // Generate tests until 0 instances is entered.
    std::cout<< "Enter the number of instances to generate: ";
    std::cin>> instances;
    while(instances != 0) {
        inst.push_back(instances);
        std::cout<<"Enter the number of jobs: ";
        std::cin>>jobs;
        job.push_back(jobs);
        std::cout<<"Enter the number of machines: ";
        std::cin>>machines;
        mach.push_back(machines);
        std::cout<< "Enter the number of instances to generate: ";
        std::cin>> instances;
    }

    // Not really random seed
    int seed = jobs * 31 + machines * 17 + instances * 7;
    std::mt19937 rng(seed);

    for(int t = 0;t < inst.size();t++) {
        instances = inst[t];
        jobs = job[t];
        machines = mach[t];
        for (int i = 0; i < instances; ++i) {
            int seed_time = unif(rng, 0, INT_MAX);
            int seed_machine = unif(rng, 0, INT_MAX);
            std::string filename = "instance_j" + std::to_string(jobs) + "m" + std::to_string(machines) + "_" + std::to_string(i+1) + ".dzn";
            generate_instance(jobs, machines, seed_time, seed_machine, filename);
            log << filename << ", " << seed_time << ", " << seed_machine << "\n";
        }
    }

    return 0;
}
