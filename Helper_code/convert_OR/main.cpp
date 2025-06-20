#include <iostream>
#include <fstream>
#include <vector>
#include <string>

std::ifstream in("input.txt");

void create_instance(int n, int m, std::string filename) {
    std::ofstream out(filename + ".dzn");
    int d,mc;
    std::vector<std::vector<int>> durations;
    std::vector<std::vector<int>> machines;

    for(int i = 0;i < n;i++) {
        std::vector<int> dur;
        std::vector<int> mach;
        for(int j = 0;j < m;j++) {
            in>>mc>>d;
            dur.push_back(d);
            mach.push_back(mc+1);
        }
        durations.push_back(dur);
        machines.push_back(mach);
    }

    std::string to_write;

    out<<"n = "<<n<<";\nm = "<<m<<";\n\n";
    out<<"d = [| ";
    for(int i = 0;i < n;i++) {
        std::vector<int> dur = durations[i];
        for(int j = 0;j < dur.size();j++) {
            if(j != dur.size() - 1)
                out<< dur[j]<<", ";
            else
                out<< dur[j]<<" |";
        }
        if(i == n - 1)
            out<<"];\n\n";
        else
            out<<"\n";
    }
    out<<"mc = [| ";
    for(int i = 0;i < n;i++) {
        std::vector<int> mach = machines[i];
        for(int j = 0;j < mach.size();j++) {
            if(j != mach.size() - 1)
                out<< mach[j]<<", ";
            else
                out<< mach[j]<<" |";
        }
        if(i == n - 1)
            out<<"];";
        else
            out<<"\n";
    }
}

int main()
{
    int n,m;
    std::string instance_name;
    in>>instance_name;
    while(instance_name != "EOF") {
        in>>n;
        in>>m;
        create_instance(n,m,instance_name);
        in>>instance_name;
    }
    return 0;
}
